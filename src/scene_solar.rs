use std::f64::consts::TAU;

use crate::camera::Camera;
use crate::light_base::Illuminate;
use crate::material::Material;
use crate::normal_map::ImageNormalMap;
use crate::object_base::{Intersect, ObjectList};
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
// Planet radii are inflated ~200× relative to orbital distances so they
// are clearly visible (same approach as NASA educational visualisations).

const EARTH_ORBIT: f64 = 2.0;
const MARS_ORBIT:  f64 = 3.04;   // 1.52 × Earth
const MOON_ORBIT:  f64 = 0.25;   // from Earth centre (real 0.00257 AU, inflated)

const EARTH_R: f64 = 0.22;
const MOON_R:  f64 = 0.060;      // 0.27 × Earth diameter
const MARS_R:  f64 = 0.117;      // 0.53 × Earth diameter

// ---------------------------------------------------------------------------
// Periods (screen seconds)
// ---------------------------------------------------------------------------
// 1 Earth year = 30 s → all other periods follow from the real ratios.

const EARTH_YEAR:  f64 = 1800.0;  // 30 minutes per Earth year
const MARS_YEAR:   f64 = EARTH_YEAR * 1.8809;   // Mars year ≈ 1.88 Earth years
const MOON_MONTH:  f64 = EARTH_YEAR / 12.37;    // 12.37 lunar months per year
const EARTH_DAY:   f64 = EARTH_YEAR / 365.25;   // Earth self-rotation

// ---------------------------------------------------------------------------
// SolarScene
// ---------------------------------------------------------------------------

pub struct SolarScene {
    pub camera: Camera,
    pub objects: ObjectList,
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
            Transform::new(Vector3::new(EARTH_ORBIT, 0.0, 0.0), Vector3::repeat(EARTH_R)),
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
            Transform::new(
                Vector3::new(EARTH_ORBIT + MOON_ORBIT, 0.0, 0.0),
                Vector3::repeat(MOON_R),
            ),
            Material::new(
                ImageTexture::load("textures/2k_moon.jpg").expect("moon texture"),
            )
            .with_specular(0.1, 8.0),
        )));

        // --- Mars ---
        let mars_idx = objects.len();
        objects.push(Box::new(Sphere::new(
            Transform::new(Vector3::new(MARS_ORBIT, 0.0, 0.0), Vector3::repeat(MARS_R)),
            Material::new(
                ImageTexture::load("textures/2k_mars.jpg").expect("mars texture"),
            )
            .with_specular(0.2, 8.0),
        )));

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

        SolarScene {
            camera,
            objects: ObjectList::from(objects),
            lights: vec![
                // Sun: a bright point light at the origin.
                // No shadow is tested against the Sun itself (no geometry), so
                // planets are correctly lit/shadowed by each other only.
                Box::new(PointLight::new(Point3::origin(), 2.0)),
                // Very dim ambient — space is dark.
                Box::new(AmbientLight::new(0.02)),
            ],
            x_size: 1280,
            y_size: 720,
            earth_idx,
            moon_idx,
            mars_idx,
        }
    }

    /// Moves Earth, Moon, and Mars to their positions at `time` screen-seconds.
    pub fn animate(&mut self, time: f64) {
        // Earth orbits the Sun and self-rotates.
        let earth_angle = time * TAU / EARTH_YEAR;
        let earth_pos = Vector3::new(
            EARTH_ORBIT * earth_angle.cos(),
            EARTH_ORBIT * earth_angle.sin(),
            0.0,
        );
        let earth_spin = UnitQuaternion::from_axis_angle(
            &Vector3::z_axis(),
            time * TAU / EARTH_DAY,
        );
        {
            let t = self.objects.get_mut(self.earth_idx).unwrap().transform_mut();
            t.set_translation(earth_pos);
            t.set_rotation(earth_spin);
        }

        // Moon orbits Earth (in the same ecliptic plane for simplicity).
        let moon_angle = time * TAU / MOON_MONTH;
        let moon_pos = earth_pos + Vector3::new(
            MOON_ORBIT * moon_angle.cos(),
            MOON_ORBIT * moon_angle.sin(),
            0.0,
        );
        self.objects.get_mut(self.moon_idx).unwrap().transform_mut()
            .set_translation(moon_pos);

        // Mars orbits the Sun more slowly.
        let mars_angle = time * TAU / MARS_YEAR;
        let mars_pos = Vector3::new(
            MARS_ORBIT * mars_angle.cos(),
            MARS_ORBIT * mars_angle.sin(),
            0.0,
        );
        self.objects.get_mut(self.mars_idx).unwrap().transform_mut()
            .set_translation(mars_pos);

        self.objects.refresh_bounds();
    }

    fn trace_inner(&self, ray: &Ray, depth: u32, weight: f64) -> Color {
        match self.objects.closest_hit(ray) {
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
