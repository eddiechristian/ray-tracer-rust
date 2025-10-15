use crate::vec3::Vec3;

#[derive(Clone, Copy, Debug, Default)] 
pub struct Ray {
    pub point1: Vec3,
    pub point2: Vec3,
    pub lab: Vec3,
}

#[derive(Clone, Copy, Debug, Default)] 
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Ray {
    /// Creates a new Ray with the given points and lab values.
    pub fn new(point1: Vec3, point2: Vec3) -> Self {
        let lab = point2 - point1;
        Self { point1, point2, lab }
    }
}
