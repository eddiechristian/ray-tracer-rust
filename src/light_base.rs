use crate::object_base::IntersectionResult;


	#[derive(Clone, Copy, Debug, Default)] 
pub struct IlluminationResult {
    pub intensity: f64,
}

pub trait Illuminate {
    fn compute_illumination(&self, intersection_result: &IntersectionResult) -> Option<IlluminationResult>;
}