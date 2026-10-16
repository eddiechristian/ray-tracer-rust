use nalgebra::Vector3;
use crate::object_base::{IntersectionResult, ObjectList};


	#[derive(Clone, Copy, Debug, Default)] 
pub struct IlluminationResult {
    pub intensity: f64,
    /// Unit vector from the surface toward the light, for specular highlights.
    /// None for lights with no direction (ambient).
    pub direction: Option<Vector3<f64>>,
}

pub trait Illuminate: Send + Sync {
    /// `objects` is the whole scene, so lights can check whether anything casts a shadow.
    /// `normal` is the (possibly bump/normal-map-perturbed) surface normal to use for
    /// diffuse shading. The unperturbed `intersection_result.local_normal` is still used
    /// for shadow ray offset so self-shadowing stays correct.
    fn compute_illumination(
        &self,
        intersection_result: &IntersectionResult,
        normal: &Vector3<f64>,
        objects: &ObjectList,
    ) -> Option<IlluminationResult>;
}
