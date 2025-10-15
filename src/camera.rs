use nalgebra::Vector3;
use crate::ray::Ray;

pub struct Camera {
    pub position: Vector3<f64>,
    pub look_at: Vector3<f64>,
    pub up: Vector3<f64>,
    pub length: f64,
    pub horiz_size: f64,
    pub aspect_ratio: f64,
    pub alignment_vec: Vector3<f64>,
    pub proj_screen_u: Vector3<f64>,
    pub proj_screen_v: Vector3<f64>,
    pub proj_screen_center: Vector3<f64>,
}

pub const DEFAULT_POS: Vector3<f64> = Vector3::new(0.0, -10.0, 0.0);
pub const DEFAULT_LOOK_AT: Vector3<f64> = Vector3::new(0.0, 0.0, 0.0);
pub const DEFAULT_HORIZ_SZ: f64 = 0.25;
pub const DEFAULT_CAMERA_LEN: f64 = 1.0;
pub const DEFAULT_ASPECT_RATIO: f64 = 1280.0/ 720.0;

impl Default for Camera {
    fn default() -> Self {
        let align_vec: Vector3<f64> = (DEFAULT_LOOK_AT - DEFAULT_POS).normalize();
        let camera_up: Vector3<f64> = Vector3::new(0.0, 0.0, 1.0);
        let screen_u = align_vec.cross(&camera_up).normalize();
        let screen_v = screen_u.cross(&align_vec);

        Camera {
            position: DEFAULT_POS,
            look_at: DEFAULT_LOOK_AT,
            up: camera_up,
            length: DEFAULT_CAMERA_LEN,
            horiz_size: DEFAULT_HORIZ_SZ,
            aspect_ratio: DEFAULT_ASPECT_RATIO,
            alignment_vec: align_vec,
            proj_screen_u: screen_u * DEFAULT_HORIZ_SZ,
            proj_screen_v: screen_v * (DEFAULT_HORIZ_SZ / DEFAULT_ASPECT_RATIO),
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
