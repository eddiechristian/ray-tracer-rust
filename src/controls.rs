// Keyboard and mouse → `ShipInput` / `CameraInput`. This is the only device-specific part of
// flying the ship: a game controller will fill the same inputs from its sticks and triggers.
//
//   Ship:   W/S pitch (W = nose down, as pushing a stick forward), A/D yaw, Q/E roll,
//           Shift thrust, Ctrl brake (reverse thrust).
//   Camera: drag with the right mouse button to orbit, wheel to zoom; or the arrow keys to
//           orbit and PageUp/PageDown to zoom. C switches free / chase camera.

use std::collections::HashSet;
use winit::keyboard::KeyCode;
use crate::chase_camera::CameraInput;
use crate::ship::ShipInput;

/// Radians of camera swing per pixel of mouse drag.
pub const DRAG_RADIANS_PER_PIXEL: f64 = 0.006;
/// Zoom factor per wheel notch.
pub const WHEEL_ZOOM: f64 = 1.12;

#[derive(Default)]
pub struct Controls {
    held: HashSet<KeyCode>,
}

impl Controls {
    pub fn key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            self.held.insert(key);
        } else {
            self.held.remove(&key);
        }
    }

    /// Forgets held keys, e.g. when the window loses focus and key releases would be missed.
    pub fn release_all(&mut self) {
        self.held.clear();
    }

    /// +1 if `plus` is held, -1 if `minus` is, 0 for both or neither.
    fn axis(&self, plus: &[KeyCode], minus: &[KeyCode]) -> f64 {
        let down = |keys: &[KeyCode]| keys.iter().any(|k| self.held.contains(k));
        f64::from(u8::from(down(plus))) - f64::from(u8::from(down(minus)))
    }

    pub fn ship_input(&self) -> ShipInput {
        use KeyCode::*;
        ShipInput {
            pitch: self.axis(&[KeyS], &[KeyW]),
            yaw: self.axis(&[KeyA], &[KeyD]),
            roll: self.axis(&[KeyE], &[KeyQ]),
            thrust: self.axis(&[ShiftLeft, ShiftRight], &[ControlLeft, ControlRight]),
        }
    }

    pub fn camera_input(&self) -> CameraInput {
        use KeyCode::*;
        CameraInput {
            orbit_x: self.axis(&[ArrowRight], &[ArrowLeft]),
            orbit_y: self.axis(&[ArrowUp], &[ArrowDown]),
            zoom: self.axis(&[PageUp], &[PageDown]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_axes() {
        let mut c = Controls::default();
        assert_eq!(c.ship_input(), ShipInput::default());
        c.key(KeyCode::KeyW, true);
        c.key(KeyCode::ShiftLeft, true);
        c.key(KeyCode::ArrowLeft, true);
        assert_eq!(c.ship_input(), ShipInput { pitch: -1.0, thrust: 1.0, ..Default::default() });
        assert_eq!(c.camera_input(), CameraInput { orbit_x: -1.0, ..Default::default() });
        // Opposite keys cancel; releasing works.
        c.key(KeyCode::KeyS, true);
        assert_eq!(c.ship_input().pitch, 0.0);
        c.key(KeyCode::KeyW, false);
        assert_eq!(c.ship_input().pitch, 1.0);
        c.release_all();
        assert_eq!(c.ship_input(), ShipInput::default());
    }
}
