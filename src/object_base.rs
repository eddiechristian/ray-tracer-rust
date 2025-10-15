use crate::vec3::Vec3;
use crate::ray::Ray;
// use crate::ray::Color;

#[derive(Clone, Copy, Debug, Default)] 
pub struct IntersectionResult {
    pub intersection: Vec3,
    pub local_normal: Vec3,
}

pub trait Intersect {
    fn test_intersection(&self, ray: &Ray) -> Option<IntersectionResult>;
}