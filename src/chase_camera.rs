// Camera rig that follows a target (the ship) but is steered on its own: it sits on a sphere
// around the target at `azimuth` / `elevation` / `distance` and always looks at the target.
// The target's motion moves the camera along with it but never turns it, so you can swing the
// camera behind, beside, above or below the ship while it flies.
//
// In `Free` mode the angles are measured from the scene's fixed axes. In `Chase` mode they're
// measured from the ship's own axes, so "behind" stays behind through turns, loops and rolls.

use nalgebra::{UnitQuaternion, Vector3};
use crate::camera::Camera;

/// One frame's camera controls, each in [-1, 1], like stick values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// +1 = swing around the target to the right (as seen from the camera).
    pub orbit_x: f64,
    /// +1 = swing up over the target.
    pub orbit_y: f64,
    /// +1 = move closer.
    pub zoom: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Angles fixed to the scene: the ship turns while the view stays put.
    Free,
    /// Angles fixed to the ship: the view turns with it.
    Chase,
}

// Full-input rates, radians per second, and zoom factor per second.
const ORBIT_RATE: f64 = 1.5;
const ZOOM_RATE: f64 = 1.0; // ×e (2.7×) closer per second at full input
// Just short of straight above or below, where the look direction would equal "up".
const MAX_ELEVATION: f64 = 89.0 * std::f64::consts::PI / 180.0;
// Default zoom limits, for a rig around the shuttle (see `with_distance_limits`).
const MIN_DISTANCE: f64 = 0.35;
const MAX_DISTANCE: f64 = 6.0;

pub struct ChaseCamera {
    /// Angle around the target's vertical, 0 = behind (on the -x side).
    pub azimuth: f64,
    /// Angle above the target's horizontal (negative = below).
    pub elevation: f64,
    pub distance: f64,
    pub mode: Mode,
    /// Field of view, as the camera's `horiz_size`.
    pub horiz_size: f64,
    min_distance: f64,
    max_distance: f64,
}

impl ChaseCamera {
    pub fn new(azimuth: f64, elevation: f64, distance: f64, horiz_size: f64) -> Self {
        ChaseCamera {
            azimuth, elevation, distance, mode: Mode::Free, horiz_size,
            min_distance: MIN_DISTANCE, max_distance: MAX_DISTANCE,
        }
    }

    /// A free-mode rig whose camera starts at `offset` from the target (in the scene's axes).
    pub fn from_offset(offset: Vector3<f64>, horiz_size: f64) -> Self {
        let mut rig = Self::new(0.0, 0.0, 1.0, horiz_size);
        rig.max_distance = rig.max_distance.max(offset.norm());
        rig.set_local_offset(offset);
        rig
    }

    /// Camera position relative to the target, in the rig's frame (scene axes in free mode,
    /// the target's axes in chase mode).
    fn local_offset(&self) -> Vector3<f64> {
        let (sin_az, cos_az) = self.azimuth.sin_cos();
        let (sin_el, cos_el) = self.elevation.sin_cos();
        // Azimuth 0 is behind (-x); positive azimuth swings toward -y, which is the camera's
        // right when it looks forward along +x.
        Vector3::new(-cos_el * cos_az, -cos_el * sin_az, sin_el) * self.distance
    }

    /// Inverse of `local_offset`.
    fn set_local_offset(&mut self, offset: Vector3<f64>) {
        let distance = offset.norm();
        self.distance = distance.clamp(self.min_distance, self.max_distance);
        self.elevation = (offset.z / distance).asin().clamp(-MAX_ELEVATION, MAX_ELEVATION);
        self.azimuth = (-offset.y).atan2(-offset.x).rem_euclid(std::f64::consts::TAU);
    }

    /// The rig's frame: the scene's axes in free mode, the target's in chase mode.
    fn frame(&self, target_orientation: &UnitQuaternion<f64>) -> UnitQuaternion<f64> {
        match self.mode {
            Mode::Free => UnitQuaternion::identity(),
            Mode::Chase => *target_orientation,
        }
    }

    /// Zoom range, for targets bigger or smaller than the shuttle.
    pub fn with_distance_limits(mut self, min: f64, max: f64) -> Self {
        self.min_distance = min;
        self.max_distance = max;
        self.distance = self.distance.clamp(min, max);
        self
    }

    /// Steers the rig at stick-like rates for `dt` seconds.
    pub fn update(&mut self, input: &CameraInput, dt: f64) {
        let clamp = |v: f64| v.clamp(-1.0, 1.0);
        self.rotate(clamp(input.orbit_x) * ORBIT_RATE * dt, clamp(input.orbit_y) * ORBIT_RATE * dt);
        self.zoom((ZOOM_RATE * dt * clamp(input.zoom)).exp());
    }

    /// Swings the camera by the given angles (radians), e.g. from a mouse drag.
    pub fn rotate(&mut self, d_azimuth: f64, d_elevation: f64) {
        self.azimuth = (self.azimuth + d_azimuth).rem_euclid(std::f64::consts::TAU);
        self.elevation = (self.elevation + d_elevation).clamp(-MAX_ELEVATION, MAX_ELEVATION);
    }

    /// Moves closer by `factor` (> 1 = closer, < 1 = further), e.g. from the mouse wheel.
    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance / factor).clamp(self.min_distance, self.max_distance);
    }

    /// Switches free / chase without moving the camera: its angles are re-expressed in the
    /// new frame. (Entering chase, "up" becomes the target's up.)
    pub fn set_mode(&mut self, mode: Mode, target_orientation: &UnitQuaternion<f64>) {
        let world_offset = self.frame(target_orientation) * self.local_offset();
        self.mode = mode;
        let local = self.frame(target_orientation).inverse() * world_offset;
        self.set_local_offset(local);
    }

    pub fn toggle_mode(&mut self, target_orientation: &UnitQuaternion<f64>) {
        let mode = match self.mode {
            Mode::Free => Mode::Chase,
            Mode::Chase => Mode::Free,
        };
        self.set_mode(mode, target_orientation);
    }

    /// The camera looking at a target at `target` with rotation `target_orientation`.
    pub fn camera(&self, target: Vector3<f64>, target_orientation: &UnitQuaternion<f64>) -> Camera {
        let frame = self.frame(target_orientation);
        let mut camera = Camera::new(target + frame * self.local_offset(), target, frame * Vector3::z());
        camera.set_horiz_size(self.horiz_size);
        camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn close(a: Vector3<f64>, b: Vector3<f64>) -> bool {
        (a - b).norm() < 1e-9
    }

    #[test]
    fn sits_behind_beside_and_above() {
        let target = Vector3::new(1.0, 2.0, 3.0);
        let level = UnitQuaternion::identity();
        let mut rig = ChaseCamera::new(0.0, 0.0, 2.0, 0.3);
        assert!(close(rig.camera(target, &level).position(), target + Vector3::new(-2.0, 0.0, 0.0)), "behind");
        rig.rotate(FRAC_PI_2, 0.0);
        assert!(close(rig.camera(target, &level).position(), target + Vector3::new(0.0, -2.0, 0.0)), "right side");
        // Straight up is clamped to 89°: almost directly above.
        rig.rotate(0.0, 10.0);
        let above = rig.camera(target, &level).position() - target;
        assert!(above.z > 1.999 && above.xy().norm() < 0.04, "above: {above:?}");
        rig.rotate(0.0, -20.0);
        assert!((rig.camera(target, &level).position() - target).z < -1.999, "below");
        assert_eq!(rig.camera(target, &level).look_at(), target);
    }

    #[test]
    fn free_mode_ignores_the_ships_heading_and_chase_mode_follows_it() {
        let target = Vector3::zeros();
        let level = UnitQuaternion::identity();
        let turned = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), FRAC_PI_2); // nose to +y
        let mut rig = ChaseCamera::new(0.0, 0.0, 2.0, 0.3);
        assert!(close(rig.camera(target, &turned).position(), Vector3::new(-2.0, 0.0, 0.0)), "free: unchanged");
        // Entering chase behind a level ship, then the ship turns: the camera turns with it.
        rig.toggle_mode(&level);
        assert_eq!(rig.mode, Mode::Chase);
        assert!(close(rig.camera(target, &turned).position(), Vector3::new(0.0, -2.0, 0.0)), "chase: behind the nose");
    }

    #[test]
    fn switching_mode_doesnt_move_the_camera() {
        let target = Vector3::new(0.5, -1.0, 2.0);
        let ship = UnitQuaternion::from_euler_angles(0.4, -0.5, 1.2);
        let mut rig = ChaseCamera::new(0.7, 0.3, 1.5, 0.3);
        let before = rig.camera(target, &ship).position();
        rig.toggle_mode(&ship);
        assert_eq!(rig.mode, Mode::Chase);
        assert!(close(rig.camera(target, &ship).position(), before), "into chase");
        rig.toggle_mode(&ship);
        assert!(close(rig.camera(target, &ship).position(), before), "back to free");
    }

    #[test]
    fn camera_follows_the_target() {
        let rig = ChaseCamera::new(0.4, 0.3, 1.5, 0.3);
        let level = UnitQuaternion::identity();
        let a = rig.camera(Vector3::zeros(), &level).position();
        let b = rig.camera(Vector3::new(5.0, -1.0, 2.0), &level).position();
        assert!(close(b - a, Vector3::new(5.0, -1.0, 2.0)));
    }

    #[test]
    fn from_offset_reproduces_the_offset() {
        for offset in [Vector3::new(-0.25, -0.8, 0.45), Vector3::new(-1.5, -5.5, 3.5), Vector3::new(2.0, 1.0, -0.5)] {
            let rig = ChaseCamera::from_offset(offset, 0.3);
            let camera = rig.camera(Vector3::new(1.0, 1.0, 1.0), &UnitQuaternion::identity());
            assert!(close(camera.position() - Vector3::new(1.0, 1.0, 1.0), offset), "{offset:?}");
        }
    }

    #[test]
    fn rates_and_zoom_limits() {
        let mut rig = ChaseCamera::new(0.0, 0.0, 1.0, 0.3);
        rig.update(&CameraInput { orbit_x: 1.0, orbit_y: 0.5, zoom: 0.0 }, 0.2);
        assert!((rig.azimuth - ORBIT_RATE * 0.2).abs() < 1e-12);
        assert!((rig.elevation - ORBIT_RATE * 0.1).abs() < 1e-12);
        rig.zoom(1000.0);
        assert_eq!(rig.distance, MIN_DISTANCE);
        rig.zoom(0.0001);
        assert_eq!(rig.distance, MAX_DISTANCE);
    }
}
