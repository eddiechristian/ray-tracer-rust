use std::f64::consts::TAU;

use crate::camera::Camera;
use crate::light_base::Illuminate;
use crate::material::Material;
use crate::normal_map::ImageNormalMap;
use crate::object_base::{Intersect, IntersectionResult, ObjectList};
use crate::point_light::PointLight;
use crate::ambient_light::AmbientLight;
use crate::ray::{Color, Ray};
use crate::scene::SceneContext;
use crate::sphere::Sphere;
use crate::texture::ImageTexture;
use crate::transform::Transform;
use nalgebra::{Point3, UnitQuaternion, Vector3};
use rayon::prelude::*;

// ---------------------------------------------------------------------------
// Scale
// ---------------------------------------------------------------------------
// Orbits are in world units; 1 unit ≈ 1 AU compressed for visibility.
// Planet radii are inflated ~2600× relative to orbital distances so they
// are clearly visible (same approach as NASA educational visualisations).
// Relative planet sizes and the Earth/Mars orbit ratio are true to life.

const AU: f64 = 2.0;             // world units per astronomical unit

const SUN_R:   f64 = 0.5;        // real Sun is ~109× Earth; shrunk so it doesn't swallow the orbits
const EARTH_R: f64 = 0.22;
const MOON_R:  f64 = 0.060;      // 0.27 × Earth diameter
const MARS_R:  f64 = 0.117;      // 0.53 × Earth diameter

// ---------------------------------------------------------------------------
// Periods (screen seconds)
// ---------------------------------------------------------------------------
// 1 Earth year = 1800 s (30 min) → all other periods follow from the real ratios.
// (The camera's own orbit is separate: ORBIT_PERIOD_SECS in main.rs.)
// Orbits and spins are computed in fixed (star) axes, so these are sidereal periods.

const EARTH_YEAR: f64 = 1800.0;                // 30 minutes per Earth year
const EARTH_DAY:  f64 = EARTH_YEAR / 366.256;  // 23.93 h sidereal day
const MARS_DAY:   f64 = EARTH_DAY * 1.02749;   // 24.62 h sidereal day

// ---------------------------------------------------------------------------
// Orientation (radians)
// ---------------------------------------------------------------------------
// The ecliptic (Earth's orbit plane) is z = 0 and +x points to the vernal equinox,
// so Earth's north pole leans toward +y: the northern summer is at Earth longitude 270°.
// time = 0 is the J2000 epoch (1 Jan 2000, 12:00), so the bodies start where they really were.

const DEG: f64 = std::f64::consts::PI / 180.0;

const EARTH_TILT: f64 = 23.44 * DEG;   // axial tilt (obliquity)
const MARS_TILT:  f64 = 25.19 * DEG;
const EARTH_SPIN_AT_EPOCH: f64 = 280.46 * DEG; // sidereal time: puts Greenwich at local noon

// ---------------------------------------------------------------------------
// Keplerian orbits
// ---------------------------------------------------------------------------

/// Elliptical orbit from its classical elements at J2000. The line of nodes and the
/// perihelion may rotate slowly (period `f64::INFINITY` = fixed; negative = backwards).
struct Orbit {
    semi_major: f64,
    eccentricity: f64,
    inclination: f64,
    /// Longitude where the body crosses the reference plane going north.
    node: f64,
    node_period: f64,
    /// Longitude of the closest point to the focus (node + argument of perihelion).
    perihelion: f64,
    perihelion_period: f64,
    /// Mean longitude at time 0 (where a uniformly moving body would be).
    mean_longitude: f64,
    period: f64,
}

/// Earth (Earth–Moon barycentre) around the Sun; it defines the ecliptic.
const EARTH_ORBIT: Orbit = Orbit {
    semi_major: 1.00000 * AU,
    eccentricity: 0.01671,
    inclination: 0.0,
    node: 0.0,
    node_period: f64::INFINITY,
    perihelion: 102.94 * DEG,
    perihelion_period: f64::INFINITY,
    mean_longitude: 100.46 * DEG,
    period: EARTH_YEAR,
};

/// Mars around the Sun: distance swings 1.38–1.67 AU.
const MARS_ORBIT: Orbit = Orbit {
    semi_major: 1.52371 * AU,
    eccentricity: 0.09339,
    inclination: 1.850 * DEG,
    node: 49.56 * DEG,
    node_period: f64::INFINITY,
    perihelion: 336.06 * DEG,
    perihelion_period: f64::INFINITY,
    mean_longitude: -4.55 * DEG,
    period: EARTH_YEAR * 1.88085,
};

/// The Moon around Earth. Its distance isn't to scale (real: 60 Earth radii), but the
/// shape of its orbit is: tilted 5.14° (so eclipses are rare), with the plane regressing
/// every 18.6 years and the perigee advancing every 8.85 years.
const MOON_ORBIT: Orbit = Orbit {
    semi_major: 0.4, // must stay above EARTH_R + MOON_R at perigee
    eccentricity: 0.0549,
    inclination: 5.145 * DEG,
    node: 125.08 * DEG,
    node_period: -18.61 * EARTH_YEAR,
    perihelion: 83.35 * DEG,
    perihelion_period: 8.85 * EARTH_YEAR,
    mean_longitude: 218.32 * DEG,
    period: EARTH_YEAR / 13.369, // 27.32-day sidereal month
};

impl Orbit {
    fn node_at(&self, time: f64) -> f64 {
        self.node + time * TAU / self.node_period
    }

    fn mean_longitude_at(&self, time: f64) -> f64 {
        self.mean_longitude + time * TAU / self.period
    }

    /// Rotation taking the orbit's own xy-plane (x toward the ascending node) to world space.
    fn plane(&self, time: f64) -> UnitQuaternion<f64> {
        UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.node_at(time))
            * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.inclination)
    }

    /// Position relative to the body orbited (at the focus). Moves fastest near perihelion.
    fn position(&self, time: f64) -> Vector3<f64> {
        let e = self.eccentricity;
        let perihelion = self.perihelion + time * TAU / self.perihelion_period;
        // Mean anomaly grows uniformly; solve Kepler's equation M = E - e sin E for E (Newton).
        let mean = (self.mean_longitude_at(time) - perihelion).rem_euclid(TAU);
        let mut ecc_anomaly = mean;
        for _ in 0..5 {
            ecc_anomaly -= (ecc_anomaly - e * ecc_anomaly.sin() - mean) / (1.0 - e * ecc_anomaly.cos());
        }
        let true_anomaly = 2.0 * f64::atan2(
            (1.0 + e).sqrt() * (ecc_anomaly / 2.0).sin(),
            (1.0 - e).sqrt() * (ecc_anomaly / 2.0).cos(),
        );
        let radius = self.semi_major * (1.0 - e * ecc_anomaly.cos());
        // Angle in the orbit plane measured from the ascending node.
        let angle = perihelion - self.node_at(time) + true_anomaly;
        self.plane(time) * Vector3::new(radius * angle.cos(), radius * angle.sin(), 0.0)
    }
}

/// Calendar date at `time` screen-seconds after J2000 (1 Jan 2000, 12:00),
/// e.g. "14 Mar 2000". One orbit of Earth is 365.256 solar days.
pub fn date_label(time: f64) -> String {
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let days_since_2000 = (0.5 + time / EARTH_YEAR * 365.256).floor() as i64;
    let (year, month, day) = civil_from_days(days_since_2000 + 10_957); // 1970-01-01 → 2000-01-01
    format!("{day:2} {} {year}", MONTHS[month as usize - 1])
}

/// (year, month 1–12, day 1–31) of the day `days` after 1970-01-01, in the Gregorian calendar.
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);                                   // day of era
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // year of era
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);                 // day of year, from 1 Mar
    let mp = (5 * doy + 2) / 153;                                      // month, from March = 0
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Axial tilt that leans the north pole (+z) toward +y by `tilt`.
fn axial_tilt(tilt: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -tilt)
}

/// Rotation about +z by `angle`.
fn spin(angle: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), angle)
}

// ---------------------------------------------------------------------------
// SolarScene
// ---------------------------------------------------------------------------

pub struct SolarScene {
    pub camera: Camera,
    /// Planets and moons. Only these cast shadows (see `scene_objects`).
    pub objects: ObjectList,
    /// The Sun is kept apart from `objects`: the point light sits at its centre, so if it
    /// were in the shadow test every shadow ray would hit it and the planets would go dark.
    pub sun: ObjectList,
    pub lights:  Vec<Box<dyn Illuminate>>,
    pub x_size:  u32,
    pub y_size:  u32,
    earth_idx: usize,
    moon_idx:  usize,
    mars_idx:  usize,
}

impl SolarScene {
    pub fn new() -> Self {
        let mut objects: Vec<Box<dyn Intersect>> = Vec::new();

        // --- Earth ---
        let earth_idx = objects.len();
        objects.push(Box::new(Sphere::new(
            Transform::new(Vector3::zeros(), Vector3::repeat(EARTH_R)),
            Material::new(
                ImageTexture::load("textures/2k_earth_daymap.jpg").expect("earth daymap"),
            )
            .with_specular(0.5, 16.0)
            .with_normal_map(
                ImageNormalMap::load("textures/2k_earth_normal_map.tif")
                    .expect("earth normal map")
                    .with_amplitude(3.0),
            ),
        )));

        // --- Moon ---
        let moon_idx = objects.len();
        objects.push(Box::new(Sphere::new(
            Transform::new(Vector3::zeros(), Vector3::repeat(MOON_R)),
            Material::new(
                ImageTexture::load("textures/2k_moon.jpg").expect("moon texture"),
            )
            .with_specular(0.1, 8.0),
        )));

        // --- Mars ---
        let mars_idx = objects.len();
        objects.push(Box::new(Sphere::new(
            Transform::new(Vector3::zeros(), Vector3::repeat(MARS_R)),
            Material::new(
                ImageTexture::load("textures/2k_mars.jpg").expect("mars texture"),
            )
            .with_specular(0.2, 8.0),
        )));

        // --- Sun --- self-lit, so it glows with its texture regardless of the lights.
        let sun: Vec<Box<dyn Intersect>> = vec![Box::new(Sphere::new(
            Transform::new(Vector3::zeros(), Vector3::repeat(SUN_R)),
            Material::new(ImageTexture::load("textures/2k_sun.jpg").expect("sun texture"))
                .with_emissive(),
        ))];

        // Camera: above the ecliptic plane, looking at the origin (Sun).
        // The orbit button will circle it around the Z axis at this radius.
        // Camera above the ecliptic, looking at a point between Earth and Mars
        // so all three bodies fit in frame at a comfortable zoom level.
        let mut camera = Camera::new(
            Vector3::new(0.0, -5.5, 3.5),
            Vector3::new(1.5, 0.0, 0.0),  // midpoint toward Earth/Mars orbit band
            Vector3::new(0.0, 0.0, 1.0),
        );
        camera.set_horiz_size(0.45);

        let mut scene = SolarScene {
            camera,
            objects: ObjectList::from(objects),
            sun: ObjectList::from(sun),
            lights: vec![
                // Sun: a bright point light at the origin. The Sun sphere isn't in
                // `objects`, so planets are lit/shadowed by each other only.
                Box::new(PointLight::new(Point3::origin(), 2.0)),
                // Very dim ambient — space is dark.
                Box::new(AmbientLight::new(0.02)),
            ],
            x_size: 1280,
            y_size: 720,
            earth_idx,
            moon_idx,
            mars_idx,
        };
        scene.animate(0.0); // place everything at its J2000 position
        scene
    }

    /// Moves Earth, Moon, and Mars to their positions at `time` screen-seconds after J2000.
    pub fn animate(&mut self, time: f64) {
        // Earth orbits the Sun and spins about its tilted axis.
        let earth_pos = EARTH_ORBIT.position(time);
        {
            let t = self.objects.get_mut(self.earth_idx).unwrap().transform_mut();
            t.set_translation(earth_pos);
            t.set_rotation(axial_tilt(EARTH_TILT) * spin(EARTH_SPIN_AT_EPOCH + time * TAU / EARTH_DAY));
        }

        // Moon: tidally locked, so it turns at its mean orbital rate and its near side
        // (longitude 0, local +x) faces Earth. On the elliptical orbit the true position runs
        // ahead of / behind that steady turn, which gives the real wobble (libration).
        {
            let plane = MOON_ORBIT.plane(time);
            let facing = MOON_ORBIT.mean_longitude_at(time) - MOON_ORBIT.node_at(time) + std::f64::consts::PI;
            let t = self.objects.get_mut(self.moon_idx).unwrap().transform_mut();
            t.set_translation(earth_pos + MOON_ORBIT.position(time));
            t.set_rotation(plane * spin(facing));
        }

        // Mars: elliptical, slightly inclined orbit; tilted spin axis.
        {
            let t = self.objects.get_mut(self.mars_idx).unwrap().transform_mut();
            t.set_translation(MARS_ORBIT.position(time));
            t.set_rotation(axial_tilt(MARS_TILT) * spin(time * TAU / MARS_DAY));
        }

        self.objects.refresh_bounds();
    }

    fn trace_inner(&self, ray: &Ray, depth: u32, weight: f64) -> Color {
        // Nearest of the planets and the Sun.
        let dist = |hit: &IntersectionResult| (hit.intersection - ray.point1).norm();
        let hit = match (self.objects.closest_hit(ray), self.sun.closest_hit(ray)) {
            (Some(p), Some(s)) => Some(if dist(&s) < dist(&p) { s } else { p }),
            (p, s) => p.or(s),
        };
        match hit {
            Some(hit) => hit.material.compute_color(self, ray, &hit, depth, weight),
            None => Color::default(), // Space is black.
        }
    }

    pub fn render(&self, buffer: &mut [u8]) {
        let x_fact = 2.0 / self.x_size as f64;
        let y_fact = 2.0 / self.y_size as f64;
        let row_bytes = (self.x_size * 4) as usize;
        buffer
            .par_chunks_exact_mut(row_bytes)
            .enumerate()
            .for_each(|(y, row)| {
                let norm_y = 1.0 - y as f64 * y_fact;
                for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                    let norm_x = x as f64 * x_fact - 1.0;
                    let ray = self.camera.generate_ray(norm_x, norm_y);
                    let color = self.trace_inner(&ray, 0, 1.0);
                    pixel[0] = (color.r * 255.0) as u8;
                    pixel[1] = (color.g * 255.0) as u8;
                    pixel[2] = (color.b * 255.0) as u8;
                    pixel[3] = 255;
                }
            });
    }
}

impl SceneContext for SolarScene {
    fn trace(&self, ray: &Ray, depth: u32, weight: f64) -> Color {
        self.trace_inner(ray, depth, weight)
    }
    fn scene_lights(&self)  -> &[Box<dyn Illuminate>] { &self.lights }
    fn scene_objects(&self) -> &ObjectList             { &self.objects }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Time (screen seconds) when `orbit` is at perihelion, for a fixed-apsis orbit.
    fn perihelion_time(orbit: &Orbit) -> f64 {
        (orbit.perihelion - orbit.mean_longitude).rem_euclid(TAU) / TAU * orbit.period
    }

    #[test]
    fn mars_orbit_is_keplerian() {
        let o = &MARS_ORBIT;
        let t0 = perihelion_time(o);
        // Perihelion and aphelion distances: a(1 - e) and a(1 + e), i.e. 1.381 and 1.666 AU.
        assert!((o.position(t0).norm() - o.semi_major * (1.0 - o.eccentricity)).abs() < 1e-9);
        assert!((o.position(t0 + o.period / 2.0).norm() - o.semi_major * (1.0 + o.eccentricity)).abs() < 1e-9);
        // Perihelion points along its longitude (measured in the ecliptic, near enough at 1.85°).
        let p = o.position(t0);
        assert!((p.y.atan2(p.x).rem_euclid(TAU) - o.perihelion).abs() < 0.5 * DEG);
        // Kepler's 2nd law: equal areas in equal times (sampled at four spots on the orbit).
        let dt = o.period / 1000.0;
        let area = |t: f64| o.position(t).cross(&o.position(t + dt)).norm() / 2.0;
        for t in [0.0, 0.25, 0.5, 0.75].map(|f| t0 + f * o.period) {
            assert!((area(t) / area(t0) - 1.0).abs() < 1e-3, "area at {t}");
        }
        // Stays in the inclined plane.
        assert!(o.position(123.0).dot(&(o.plane(0.0) * Vector3::z())).abs() < 1e-9);
    }

    #[test]
    fn moon_clears_earth() {
        let o = &MOON_ORBIT;
        let closest = (0..10_000)
            .map(|i| o.position(i as f64 * o.period / 1000.0).norm())
            .fold(f64::INFINITY, f64::min);
        assert!(closest > EARTH_R + MOON_R, "Moon at {closest} touches Earth");
    }

    #[test]
    fn date_label_counts_days_months_and_years() {
        let day = EARTH_YEAR / 365.256;
        assert_eq!(date_label(0.0), " 1 Jan 2000");
        assert_eq!(date_label(0.49 * day), " 1 Jan 2000"); // just before midnight
        assert_eq!(date_label(0.51 * day), " 2 Jan 2000");
        assert_eq!(date_label(30.0 * day), "31 Jan 2000");
        assert_eq!(date_label(31.0 * day), " 1 Feb 2000");
        assert_eq!(date_label(59.0 * day), "29 Feb 2000"); // 2000 is a leap year
        assert_eq!(date_label(60.0 * day), " 1 Mar 2000");
        assert_eq!(date_label(366.0 * day), " 1 Jan 2001");
        assert_eq!(date_label(366.0 * day + 365.0 * day), " 1 Jan 2002"); // 2001 isn't
    }

    #[test]
    fn earth_starts_at_j2000_position() {
        // On 1 Jan 2000 Earth was at heliocentric longitude ~100° and 0.983 AU (near perihelion).
        let p = EARTH_ORBIT.position(0.0);
        assert!((p.y.atan2(p.x) / DEG - 100.5).abs() < 1.0);
        assert!((p.norm() / AU - 0.983).abs() < 0.001);
    }
}
