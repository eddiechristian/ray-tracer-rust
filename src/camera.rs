use crate::vec3::Vec3;
use crate::vec3::DEFAULT_POS;
use crate::vec3::VEC3_ZERO;
use crate::ray::Ray;

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

pub const DEFAULT_HORIZ_SZ: f64 = 0.25;
pub const DEFAULT_CAMERA_LEN: f64 = 1.0;
pub const DEFAULT_ASPECT_RATIO: f64 = 1280.0/ 720.0;

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


// bool qbRT::Camera::GenerateRay(float proScreenX, float proScreenY, qbRT::Ray &cameraRay)
// {
// 	// Compute the location of the screen point in world coordinates.
// 	qbVector<double> screenWorldPart1 = m_projectionScreenCentre + (m_projectionScreenU * proScreenX);
// 	qbVector<double> screenWorldCoordinate = screenWorldPart1 + (m_projectionScreenV * proScreenY);
// 	
// 	// Use this point along with the camera position to compute the ray.
// 	cameraRay.m_point1 = m_cameraPosition;
// 	cameraRay.m_point2 = screenWorldCoordinate;
// 	cameraRay.m_lab = screenWorldCoordinate - m_cameraPosition;
// 	
// 	return true;
// }

impl Camera {
    pub fn new() -> Self {
        Self::default()
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
