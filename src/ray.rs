use nalgebra::Vector3;

#[derive(Clone, Copy, Debug, Default)] 
pub struct Ray {
    pub point1: Vector3<f64>,
    pub point2: Vector3<f64>,
    pub lab: Vector3<f64>,
}

#[derive(Clone, Copy, Debug, Default)] 
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Ray {
    /// Creates a new Ray with the given points and lab values.
    pub fn new(point1: Vector3<f64>, point2: Vector3<f64>) -> Self {
        let lab = point2 - point1;
        Self { point1, point2, lab }
    }
}
