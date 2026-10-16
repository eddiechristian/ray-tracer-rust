use std::f64::consts::FRAC_PI_2;

use nalgebra::{Point3, Vector3};
use crate::light_base::Illuminate;
use crate::light_base::IlluminationResult;
use crate::object_base::{IntersectionResult, ObjectList};
use crate::ray::Ray;

// Shadow rays start this far off the surface along the normal, so rounding error
// doesn't make a surface shadow itself ("shadow acne").
const SHADOW_BIAS: f64 = 1e-6;

pub struct PointLight {
    pub location: Point3<f64>,
    pub intensity: f64,
}

impl PointLight {
    pub fn new(location: Point3<f64>, intensity: f64) -> Self {
        PointLight {
            location: location,
            intensity: intensity,
        }
    }
}

impl Illuminate for PointLight {
  fn compute_illumination(
        &self,
        intersection_result: &IntersectionResult,
        normal: &Vector3<f64>,
        objects: &ObjectList,
  ) -> Option<IlluminationResult> {
     // Vector pointing from the intersection point to the light.
     let light_dir = (self.location.coords - intersection_result.intersection).normalize();

     // Diffuse angle uses the (possibly perturbed) normal passed in.
     let angle = normal.dot(&light_dir).clamp(-1.0, 1.0).acos();

     // If the normal points away from the light, there is no illumination.
     if angle > FRAC_PI_2 {
        return None;
     }

     // Shadow ray offset uses the geometric normal to avoid self-shadowing artefacts.
     let start = intersection_result.intersection + intersection_result.local_normal * SHADOW_BIAS;
     let shadow_ray = Ray::new(start, self.location.coords);
     let light_dist = (self.location.coords - start).norm();
     // Only "is anything in the way?" matters, so use the cheap test that stops at the first blocker.
     let in_shadow = objects.occluded(&shadow_ray, light_dist);
     if in_shadow {
        return None;
     }

     Some(IlluminationResult {
        intensity: self.intensity * (1.0 - (angle / FRAC_PI_2)),
        direction: Some(light_dir),
     })
  }
}