use nalgebra::Vector3;
use crate::ray::Ray;

// Pinhole camera, as in the C++ qbRT::Camera. The projection screen sits `length` in front of
// the camera; `horiz_size` is its half-width and `aspect_ratio` sets its half-height.
//
// The fields are private: every setter recomputes the projection-screen geometry (the C++
// UpdateCameraGeometry), so it can't go stale when the camera moves.
pub struct Camera {
    position: Vector3<f64>,
    look_at: Vector3<f64>,
    up: Vector3<f64>,
    length: f64,
    horiz_size: f64,
    aspect_ratio: f64,

    // Derived by `update_geometry`:
    alignment_vec: Vector3<f64>,
    proj_screen_u: Vector3<f64>,
    proj_screen_v: Vector3<f64>,
    proj_screen_center: Vector3<f64>,
}

pub const DEFAULT_POS: Vector3<f64> = Vector3::new(0.0, -10.0, 0.0);
pub const DEFAULT_LOOK_AT: Vector3<f64> = Vector3::new(0.0, 0.0, 0.0);
pub const DEFAULT_UP: Vector3<f64> = Vector3::new(0.0, 0.0, 1.0);
pub const DEFAULT_HORIZ_SZ: f64 = 0.35;
pub const DEFAULT_CAMERA_LEN: f64 = 1.0;
pub const DEFAULT_ASPECT_RATIO: f64 = 1280.0/ 720.0;

impl Default for Camera {
    fn default() -> Self {
        Camera::new(DEFAULT_POS, DEFAULT_LOOK_AT, DEFAULT_UP)
    }
}

impl Camera {
    pub fn new(position: Vector3<f64>, look_at: Vector3<f64>, up: Vector3<f64>) -> Self {
        let mut camera = Camera {
            position,
            look_at,
            up,
            length: DEFAULT_CAMERA_LEN,
            horiz_size: DEFAULT_HORIZ_SZ,
            aspect_ratio: DEFAULT_ASPECT_RATIO,
            alignment_vec: Vector3::zeros(),
            proj_screen_u: Vector3::zeros(),
            proj_screen_v: Vector3::zeros(),
            proj_screen_center: Vector3::zeros(),
        };
        camera.update_geometry();
        camera
    }

    pub fn position(&self) -> Vector3<f64> {
        self.position
    }

    pub fn look_at(&self) -> Vector3<f64> {
        self.look_at
    }

    pub fn set_position(&mut self, position: Vector3<f64>) {
        self.position = position;
        self.update_geometry();
    }

    pub fn set_look_at(&mut self, look_at: Vector3<f64>) {
        self.look_at = look_at;
        self.update_geometry();
    }

    pub fn set_up(&mut self, up: Vector3<f64>) {
        self.up = up;
        self.update_geometry();
    }

    /// Distance from the camera to the projection screen; with `horiz_size`, sets the field of view.
    pub fn set_length(&mut self, length: f64) {
        self.length = length;
        self.update_geometry();
    }

    pub fn set_horiz_size(&mut self, horiz_size: f64) {
        self.horiz_size = horiz_size;
        self.update_geometry();
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f64) {
        self.aspect_ratio = aspect_ratio;
        self.update_geometry();
    }

    /// Recompute the projection screen from position, look-at, up and the screen size.
    fn update_geometry(&mut self) {
        self.alignment_vec = (self.look_at - self.position).normalize();
        let screen_u = self.alignment_vec.cross(&self.up).normalize();
        let screen_v = screen_u.cross(&self.alignment_vec);

        self.proj_screen_u = screen_u * self.horiz_size;
        self.proj_screen_v = screen_v * (self.horiz_size / self.aspect_ratio);
        self.proj_screen_center = self.position + (self.length * self.alignment_vec);
    }

    pub fn generate_ray(&self, proj_screen_x: f64, proj_screen_y: f64)->Ray
    {
        let screen_world_part1 = self.proj_screen_center + (self.proj_screen_u * proj_screen_x);
        let screen_world_coordinate = screen_world_part1 + (self.proj_screen_v * proj_screen_y);

        // Compute the line of sight vector (lab)
        let lab = screen_world_coordinate - self.position;

        // Construct and return the new Ray using your fields
        Ray {
            point1: self.position,
            point2: screen_world_coordinate,
            lab,
        }
    }
}
