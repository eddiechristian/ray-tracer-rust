use crate::vec3::Vec3;
use crate::ray::Ray;
use crate::object_base::Intersect;
use crate::object_base::IntersectionResult;
use crate::vec3::dot;
// 1. Define Sphere as a Unit Struct (no braces or fields)
pub struct Sphere;

impl Intersect for Sphere {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult> {
        // Since this is a Unit Sphere, we assume Center = (0,0,0) and Radius = 1.0
       
        
        let v = ray.lab.normalize();       // Direction vector
        let p = ray.point1;    // Ray origin

      
        let a = 1.0;
        let b = 2.0 * dot(&p,&v);

        let c = dot(&p,&p) - 1.0;

        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return None; // No intersection
        }

	
        // Find the closest positive intersection point t
        let t1: f64 = (-b + discriminant.sqrt()) / (2.0 * a);
        let t2: f64 = (-b - discriminant.sqrt()) / (2.0 * a);
        
        if t1 < 0.0  && t2 < 0.0{
            return None; // Hit is behind the camera
        }
        
        let t_min = t1.min(t2);
        // Calculate actual intersection point and local surface normal
        let intersection = ray.point1 + (v * t_min);
        let local_normal = intersection.normalize(); // On a unit sphere at origin, position == normal!

        Some(IntersectionResult {
            intersection,
            local_normal,
        })
    }
}
