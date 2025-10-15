use crate::object_base::{Intersect, IntersectionResult};


	#[derive(Clone, Copy, Debug, Default)] 
pub struct IlluminationResult {
    pub intensity: f64,
}

pub trait Illuminate {
    /// `objects` is the whole scene, so lights can check whether anything casts a shadow.
    fn compute_illumination(&self, intersection_result: &IntersectionResult, objects: &[Box<dyn Intersect>]) -> Option<IlluminationResult>;
}