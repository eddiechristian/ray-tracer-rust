use crate::vec3::Vec3;
use crate::vec3::DEFAULT_POS;
use crate::vec3::VEC3_ZERO;

pub struct Camera {
    pub position: Vec3,
    pub look_at: Vec3,
    pub up: Vec3,
    pub length: f64,
    pub horiz_size: f64,
    pub aspect_ratio: f64,
    pub alignment_vec: Vec3,
    pub proj_screen_u: Vec3,
    pub proj_screen_v: Vec3,
    pub proj_screen_center: Vec3,
}

pub const DEFAULT_HORIZ_SZ: f64 = 1.0;
pub const DEFAULT_CAMERA_LEN: f64 = 1.0;
pub const DEFAULT_ASPECT_RATIO: f64 = 1.0;

impl Default for Camera {
    fn default() -> Self {
        let align_vec: Vec3 = (VEC3_ZERO - DEFAULT_POS).normalize();
        let camera_up: Vec3 = Vec3::new(0.0, 0.0, 1.0);
        let screen_u = Vec3::cross(&align_vec, &camera_up).normalize();
        let screen_v = Vec3::cross(&screen_u.clone(), &align_vec.clone());

        Camera {
            position: DEFAULT_POS,
            look_at: VEC3_ZERO,
            up: camera_up,
            length: DEFAULT_CAMERA_LEN,
            horiz_size: DEFAULT_HORIZ_SZ,
            aspect_ratio: DEFAULT_ASPECT_RATIO,
            alignment_vec: align_vec.clone(),
            proj_screen_u: screen_u.clone() * DEFAULT_HORIZ_SZ,
            proj_screen_v: screen_v.clone() * (DEFAULT_HORIZ_SZ / DEFAULT_ASPECT_RATIO),
            proj_screen_center: DEFAULT_POS + (DEFAULT_CAMERA_LEN * align_vec),
        }
    }
}

impl Camera {
    fn new() -> Self {
        Self::default()
    }
}
