use crate::sphere::Sphere;
use crate::camera::Camera;
use crate::object_base::Intersect;

pub struct Scene {
    pub camera: Camera,
    pub sphere: Sphere,
    pub x_size: u32,
    pub y_size: u32,
    
}

impl Scene {

    pub fn new() -> Self {
        
        Scene { camera: Camera::default(), sphere: Sphere, x_size: 1280, y_size: 720 }
    }
    
    pub fn render(&self,  buffer: &mut [u8])
    {
        
        let x_fact: f64 = 2.0 / self.x_size as f64;
        let y_fact: f64 = 2.0 / self.y_size as f64;
        let mut min_dist: f64 = 1e6;
        let mut max_dist: f64 = 0.0;
       
        for y in 0..self.y_size {
              for x in 0..self.x_size {
                    let norm_x: f64 = x as f64 * x_fact - 1.0;
                    let norm_y: f64 = y as f64 * y_fact - 1.0;
                    let ray = self.camera.generate_ray(norm_x, norm_y);
                    let pixel_index = (((y * self.x_size) + x) * 4) as usize;
                    if let Some(intersection) = self.sphere.test_intersection(&ray) {
                            
                        let dist: f64 = (intersection.intersection - ray.point1).length();

                        let red: u8 = (255.0 - ((dist - 9.0) / 0.94605) * 255.0) as u8;
                            
                        buffer[pixel_index] = red; // R
                        buffer[pixel_index + 1] = 0; // G
                        buffer[pixel_index + 2] = 0; // B
                        buffer[pixel_index + 3] = 255; // A
                    }
              }
        }
       
    }
}