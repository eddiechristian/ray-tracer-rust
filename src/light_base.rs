use nalgebra::Vector3;
use crate::object_base::{Intersect, IntersectionResult};


	#[derive(Clone, Copy, Debug, Default)] 
pub struct IlluminationResult {
    pub intensity: f64,
    /// Unit vector from the surface toward the light, for specular highlights.
    /// None for lights with no direction (ambient).
    pub direction: Option<Vector3<f64>>,
}

pub trait Illuminate: Send + Sync {
    /// `objects` is the whole scene, so lights can check whether anything casts a shadow.
    fn compute_illumination(&self, intersection_result: &IntersectionResult, objects: &[Box<dyn Intersect>]) -> Option<IlluminationResult>;
}