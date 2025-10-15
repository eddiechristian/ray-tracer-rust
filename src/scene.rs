use crate::sphere::Sphere;
use crate::camera::Camera;
use crate::object_base::Intersect;
use crate::light_base::Illuminate;
use crate::point_light::PointLight;
use crate::ray::Color;
use nalgebra::{Point3, Vector3};

pub struct Scene {
    pub camera: Camera,
    pub sphere: Sphere,
    pub light: PointLight,
    pub x_size: u32,
    pub y_size: u32,

}

impl Scene {

    pub fn new() -> Self {

        Scene {
            camera: Camera::default(),
            sphere: Sphere::new(Vector3::new(3.0, 0.0, 0.0), 1.5, Color { r: 1.0, g: 0.0, b: 0.0 }),
            light: PointLight::new(Point3::new(5.0, -10.0, 5.0), 1.0),
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
                    let norm_y: f64 = y as f64 * y_fact - 1.0;
                    let ray = self.camera.generate_ray(norm_x, norm_y);
                    let pixel_index = (((y * self.x_size) + x) * 4) as usize;

                    // Background, and surfaces facing away from the light, are black.
                    let mut color = Color::default();

                    if let Some(intersection) = self.sphere.test_intersection(&ray) {
                        if let Some(illumination) = self.light.compute_illumination(&intersection) {
                            color = Color {
                                r: intersection.color.r * illumination.intensity,
                                g: intersection.color.g * illumination.intensity,
                                b: intersection.color.b * illumination.intensity,
                            };
                        }
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
