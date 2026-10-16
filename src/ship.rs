// The player's ship: arcade-style flight, not orbital mechanics. Turning comes from the
// pitch/yaw/roll inputs, thrust pushes along the nose, and drag slowly brings it to rest.
// Inputs come from `ShipInput`, so the keyboard now and a game controller later both drive it
// the same way. The camera is separate (see `chase_camera.rs`).
//
// Ship space matches the shuttle model: nose +x, left wing +y, up +z.

use nalgebra::{UnitQuaternion, Vector3};

/// One frame's ship controls, each in [-1, 1], like stick and trigger values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShipInput {
    /// +1 = nose up.
    pub pitch: f64,
    /// +1 = nose left.
    pub yaw: f64,
    /// +1 = roll right (right wing down).
    pub roll: f64,
    /// +1 = full thrust forward, -1 = full reverse thrust (braking).
    pub thrust: f64,
}

// Full-input turn rates, radians per second.
const PITCH_RATE: f64 = 1.2;
const YAW_RATE: f64 = 0.9;
const ROLL_RATE: f64 = 1.8;
/// Acceleration at full thrust, scene units per second².
const THRUST_ACCEL: f64 = 0.6;
/// Velocity decays as exp(-DRAG × t), so full thrust levels off at THRUST_ACCEL / DRAG
/// (0.75 units/s, about 2.5 shuttle lengths per second).
const DRAG: f64 = 0.8;
/// Longest time step simulated at once: longer frames are split into steps this size or
/// shorter, so the flight path is the same at any frame rate.
const MAX_STEP: f64 = 1.0 / 120.0;

pub struct Ship {
    pub position: Vector3<f64>,
    /// Rotation from ship space to the scene.
    pub orientation: UnitQuaternion<f64>,
    pub velocity: Vector3<f64>,
}

impl Ship {
    pub fn new(position: Vector3<f64>, orientation: UnitQuaternion<f64>) -> Self {
        Ship { position, orientation, velocity: Vector3::zeros() }
    }

    /// Direction the nose points, in the scene.
    pub fn forward(&self) -> Vector3<f64> {
        self.orientation * Vector3::x()
    }

    /// Advances the ship by `dt` seconds under `input`.
    pub fn update(&mut self, input: &ShipInput, dt: f64) {
        let steps = (dt / MAX_STEP).ceil().max(1.0);
        for _ in 0..steps as usize {
            self.step(input, dt / steps);
        }
    }

    fn step(&mut self, input: &ShipInput, dt: f64) {
        let clamp = |v: f64| v.clamp(-1.0, 1.0);

        // Turn about the ship's own axes. Positive rotation about +y dips the nose, so
        // nose-up is negative; about +x it lifts the left wing, so roll-right is positive.
        let turn = Vector3::new(
            clamp(input.roll) * ROLL_RATE,
            -clamp(input.pitch) * PITCH_RATE,
            clamp(input.yaw) * YAW_RATE,
        ) * dt;
        self.orientation *= UnitQuaternion::from_scaled_axis(turn);
        self.orientation.renormalize(); // keep rounding error from building up over many frames

        // Thrust along the nose, then drag (exact for any dt, so the frame rate doesn't matter).
        self.velocity += self.forward() * (clamp(input.thrust) * THRUST_ACCEL * dt);
        self.velocity *= (-DRAG * dt).exp();
        self.position += self.velocity * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    fn level_ship() -> Ship {
        Ship::new(Vector3::zeros(), UnitQuaternion::identity())
    }

    /// Runs exactly `seconds` of `input`, in frames of about 1/60 s.
    fn fly(ship: &mut Ship, input: ShipInput, seconds: f64) {
        let frames = (seconds * 60.0).ceil();
        for _ in 0..frames as usize {
            ship.update(&input, seconds / frames);
        }
    }

    #[test]
    fn turns_the_right_way() {
        let close = |a: Vector3<f64>, b: Vector3<f64>| (a - b).norm() < 1e-6;
        let quarter = |rate: f64| FRAC_PI_2 / rate; // seconds for a 90° turn

        let mut ship = level_ship();
        fly(&mut ship, ShipInput { pitch: 1.0, ..Default::default() }, quarter(PITCH_RATE));
        assert!(close(ship.forward(), Vector3::z()), "nose up: {:?}", ship.forward());

        let mut ship = level_ship();
        fly(&mut ship, ShipInput { yaw: 1.0, ..Default::default() }, quarter(YAW_RATE));
        assert!(close(ship.forward(), Vector3::y()), "nose left: {:?}", ship.forward());

        let mut ship = level_ship();
        fly(&mut ship, ShipInput { roll: 1.0, ..Default::default() }, quarter(ROLL_RATE));
        let right_wing = ship.orientation * -Vector3::y();
        assert!(close(right_wing, -Vector3::z()), "right wing down: {right_wing:?}");
    }

    #[test]
    fn thrust_levels_off_and_drag_stops_it() {
        let mut ship = level_ship();
        fly(&mut ship, ShipInput { thrust: 1.0, ..Default::default() }, 10.0);
        let top_speed = THRUST_ACCEL / DRAG;
        assert!((ship.velocity.norm() - top_speed).abs() < 0.01 * top_speed, "{:?}", ship.velocity);
        assert!(ship.velocity.normalize().dot(&Vector3::x()) > 0.9999, "moves along the nose");

        fly(&mut ship, ShipInput::default(), 10.0);
        assert!(ship.velocity.norm() < 0.001 * top_speed, "coasts to a stop");
    }

    #[test]
    fn frame_rate_doesnt_change_the_flight() {
        let input = ShipInput { pitch: 0.5, yaw: -0.3, roll: 0.2, thrust: 1.0 };
        let run = |fps: f64| {
            let mut ship = level_ship();
            for _ in 0..(3.0 * fps) as usize {
                ship.update(&input, 1.0 / fps);
            }
            ship
        };
        let (slow, fast) = (run(20.0), run(120.0));
        assert!((slow.position - fast.position).norm() < 1e-9, "{:?} vs {:?}", slow.position, fast.position);
        assert!(slow.orientation.angle_to(&fast.orientation) < 1e-9);
    }

    #[test]
    fn inputs_beyond_full_are_clamped() {
        let mut a = level_ship();
        let mut b = level_ship();
        a.update(&ShipInput { pitch: 1.0, thrust: 1.0, ..Default::default() }, 0.1);
        b.update(&ShipInput { pitch: 5.0, thrust: 9.0, ..Default::default() }, 0.1);
        assert_eq!(a.position, b.position);
        assert_eq!(a.orientation, b.orientation);
    }
}
