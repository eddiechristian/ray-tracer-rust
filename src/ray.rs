use std::ops::{Add, AddAssign, Mul};

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

impl Color {
    pub const fn new(r: f64, g: f64, b: f64) -> Self {
        Color { r, g, b }
    }
}

impl Add for Color {
    type Output = Color;

    fn add(self, other: Color) -> Color {
        Color::new(self.r + other.r, self.g + other.g, self.b + other.b)
    }
}

impl AddAssign for Color {
    fn add_assign(&mut self, other: Color) {
        *self = *self + other;
    }
}

impl Mul<f64> for Color {
    type Output = Color;

    fn mul(self, scalar: f64) -> Color {
        Color::new(self.r * scalar, self.g * scalar, self.b * scalar)
    }
}

// Component-wise, e.g. surface color filtered by light color.
impl Mul<Color> for Color {
    type Output = Color;

    fn mul(self, other: Color) -> Color {
        Color::new(self.r * other.r, self.g * other.g, self.b * other.b)
    }
}
