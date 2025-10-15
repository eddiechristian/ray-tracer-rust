use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub point1: Vec3,
    pub point2: Vec3,
    pub lab: Vec3,
}
