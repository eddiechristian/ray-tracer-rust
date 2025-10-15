use crate::light_base::Illuminate;
use crate::light_base::IlluminationResult;
use crate::object_base::{Intersect, IntersectionResult};

// Uniform background light that reaches every surface equally, regardless of
// position or orientation, so surfaces facing away from other lights aren't black.
pub struct AmbientLight {
    pub intensity: f64,
}

impl AmbientLight {
    pub fn new(intensity: f64) -> Self {
        AmbientLight { intensity }
    }
}

impl Illuminate for AmbientLight {
  fn compute_illumination(&self, _intersection_result: &IntersectionResult, _objects: &[Box<dyn Intersect>]) -> Option<IlluminationResult>{
     Some(IlluminationResult { intensity: self.intensity })
  }
}
