use nalgebra::Vector3;
use crate::object_base::IntersectionResult;
use crate::ray::{Color, Ray};
use crate::scene::Scene;
use crate::texture::{Flat, Texture};

// Maximum number of reflection / refraction bounces after the camera ray (the C++ uses 3,
// but a ray passing through glass spends two bounces just entering and leaving).
pub const MAX_DEPTH: u32 = 5;

// Secondary rays start this far off the surface so they don't immediately re-hit it.
const SURFACE_BIAS: f64 = 1e-6;

// Covers both the C++ SimpleMaterial and SimpleRefractive:
//   color = translucency-mix(reflectivity-mix(diffuse, reflection), refraction) + specular
pub struct Material {
    pub texture: Box<dyn Texture>,
    /// 0.0 = matte, 1.0 = perfect mirror.
    pub reflectivity: f64,
    /// Strength of the Phong specular highlight (0.0 = none).
    pub specular: f64,
    /// Phong exponent: higher = smaller, sharper highlight.
    pub shininess: f64,
    /// 0.0 = opaque, 1.0 = fully transparent.
    pub translucency: f64,
    /// Index of refraction (1.0 = air, ~1.33 water, ~1.5 glass).
    pub ior: f64,
}

impl Material {
    pub fn new(texture: impl Texture + 'static) -> Self {
        Material {
            texture: Box::new(texture),
            reflectivity: 0.0,
            specular: 0.0,
            shininess: 0.0,
            translucency: 0.0,
            ior: 1.0,
        }
    }

    /// A plain matte material of one color.
    pub fn flat(color: Color) -> Self {
        Self::new(Flat::new(color))
    }

    pub fn with_reflectivity(mut self, reflectivity: f64) -> Self {
        self.reflectivity = reflectivity;
        self
    }

    pub fn with_specular(mut self, specular: f64, shininess: f64) -> Self {
        self.specular = specular;
        self.shininess = shininess;
        self
    }

    pub fn with_translucency(mut self, translucency: f64, ior: f64) -> Self {
        self.translucency = translucency;
        self.ior = ior;
        self
    }

    /// Color seen along `ray`, which hit this material at `hit`. `depth` counts bounces so far.
    pub fn compute_color(&self, scene: &Scene, ray: &Ray, hit: &IntersectionResult, depth: u32) -> Color {
        let normal = hit.local_normal;
        let dir = ray.lab.normalize();

        // Diffuse and specular contributions from every light that reaches this point.
        let mut diffuse_intensity = 0.0;
        let mut specular_intensity = 0.0;
        for light in &scene.lights {
            if let Some(illumination) = light.compute_illumination(hit, &scene.objects) {
                diffuse_intensity += illumination.intensity;

                // Phong: compare the light's mirror direction with the direction back to the viewer.
                if let (true, Some(light_dir)) = (self.specular > 0.0, illumination.direction) {
                    let reflected = reflect(&-light_dir, &normal);
                    let alignment = reflected.dot(&-dir);
                    if alignment > 0.0 {
                        specular_intensity += self.specular * alignment.powf(self.shininess);
                    }
                }
            }
        }

        let mut color = self.texture.color(&hit.uv) * diffuse_intensity;

        if self.reflectivity > 0.0 && depth < MAX_DEPTH {
            let reflected = reflect(&dir, &normal);
            let reflection = scene.trace(&offset_ray(hit, &reflected), depth + 1);
            color = reflection * self.reflectivity + color * (1.0 - self.reflectivity);
        }

        if self.translucency > 0.0 && depth < MAX_DEPTH {
            // Total internal reflection: no refracted ray exists, so the light reflects instead.
            let refracted = refract(&dir, &normal, self.ior).unwrap_or_else(|| reflect(&dir, &normal));
            let transmission = scene.trace(&offset_ray(hit, &refracted), depth + 1);
            color = transmission * self.translucency + color * (1.0 - self.translucency);
        }

        // Highlights are white light reflected straight off the surface, added on top.
        color + Color::new(specular_intensity, specular_intensity, specular_intensity)
    }
}

/// Mirror `dir` about the surface with unit normal `normal`.
fn reflect(dir: &Vector3<f64>, normal: &Vector3<f64>) -> Vector3<f64> {
    dir - normal * (2.0 * dir.dot(normal))
}

/// Snell's law. `dir` and `normal` are unit vectors; the normal points out of the object,
/// so a ray travelling along it is leaving the object. Returns None on total internal reflection.
fn refract(dir: &Vector3<f64>, normal: &Vector3<f64>, ior: f64) -> Option<Vector3<f64>> {
    let (normal, eta) = if dir.dot(normal) < 0.0 {
        (*normal, 1.0 / ior) // entering: air -> material
    } else {
        (-normal, ior) // leaving: material -> air
    };
    let cos_i = -dir.dot(&normal);
    let k = 1.0 - eta * eta * (1.0 - cos_i * cos_i);
    if k < 0.0 {
        return None;
    }
    Some(dir * eta + normal * (eta * cos_i - k.sqrt()))
}

/// A ray leaving the hit point in `dir`, nudged off the surface on the side it's heading to.
fn offset_ray(hit: &IntersectionResult, dir: &Vector3<f64>) -> Ray {
    let side = if dir.dot(&hit.local_normal) > 0.0 { 1.0 } else { -1.0 };
    let start = hit.intersection + hit.local_normal * (side * SURFACE_BIAS);
    Ray::new(start, start + dir)
}
