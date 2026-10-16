// Space Shuttle orbiter, ray marched from signed distance functions (plan_ray_marching_shuttle.md).
//
// Local space: nose along +x, z up, wings along ±y. The orbiter is 2 units long (tail at
// x ≈ -1, nose at x = 1), so 1 unit ≈ 18.5 m: real proportions are 37 m long, 24 m wingspan
// and 17 m tall to the top of the fin.

use nalgebra::{Vector2, Vector3};
use crate::aabb::Aabb;
use crate::material::Material;
use crate::ray::Color;
use crate::ray_march::RayMarched;
use crate::sdf;
use crate::transform::Transform;

// Material indices returned by `shuttle_sdf`, in `shuttle`'s materials list.
pub const WHITE: usize = 0;   // upper surfaces (white insulation blankets)
pub const BLACK: usize = 1;   // underside, nose cap and leading edges (heat-shield tiles)
pub const ENGINE: usize = 2;  // main-engine bells
pub const WINDOW: usize = 3;  // cockpit windows

/// Fill light from the viewer's direction (see `Material::with_headlight`), so the shuttle
/// stays visible on its night side.
const HEADLIGHT: f64 = 0.35;

/// Encloses the whole shape, with a little margin (the march only happens inside it).
fn local_bounds() -> Aabb {
    Aabb::new(Vector3::new(-1.15, -0.72, -0.22), Vector3::new(1.05, 0.72, 0.7))
}

/// The orbiter as a scene object. Keep `transform`'s scale uniform (see `RayMarched::new`).
pub fn shuttle(transform: Transform) -> RayMarched {
    let materials = vec![
        Material::flat(Color::new(0.92, 0.92, 0.9)).with_specular(0.3, 16.0),
        Material::flat(Color::new(0.06, 0.06, 0.07)).with_specular(0.1, 8.0),
        Material::flat(Color::new(0.35, 0.33, 0.32)).with_specular(0.6, 32.0),
        Material::flat(Color::new(0.02, 0.03, 0.05)).with_reflectivity(0.3).with_specular(0.9, 64.0),
    ]
    .into_iter()
    .map(|material| material.with_headlight(HEADLIGHT))
    .collect();
    // The smooth blends make the SDF a bound rather than exact, so step a little short.
    RayMarched::new(transform, local_bounds(), materials, Box::new(shuttle_sdf)).with_step_scale(0.8)
}

/// Distance to the orbiter at local point `p`, and the material there.
pub fn shuttle_sdf(p: &Vector3<f64>) -> (f64, usize) {
    let m = sdf::mirror_y(p); // the orbiter is symmetric: model the +y side only

    // Fuselage: a rounded box for the cabin and payload bay, an ellipsoid for the nose, and
    // a hump for the flight deck behind the windows.
    // The nose starts well back inside the body so the two blend without a waist.
    let body = sdf::round_box(&(p - Vector3::new(-0.3, 0.0, 0.0)), &Vector3::new(0.68, 0.13, 0.13), 0.09);
    let nose = sdf::ellipsoid(&(p - Vector3::new(0.3, 0.0, -0.01)), &Vector3::new(0.7, 0.135, 0.13));
    let deck = sdf::ellipsoid(&(p - Vector3::new(0.48, 0.0, 0.06)), &Vector3::new(0.3, 0.105, 0.09));
    let fuselage = sdf::smooth_union(sdf::smooth_union(body, nose, 0.12), deck, 0.06);

    // Double-delta wings, low on the fuselage: a gently swept "glove" blending into a 45°-ish
    // main wing. The planform is a convex pentagon in (x, y), made of three triangles.
    let planform = {
        let q = Vector2::new(m.x, m.y);
        let root_front = Vector2::new(0.42, 0.08);
        let glove_end = Vector2::new(-0.15, 0.26);
        let tip_front = Vector2::new(-0.72, 0.65);
        let tip_back = Vector2::new(-0.9, 0.65);
        let root_back = Vector2::new(-0.93, 0.08);
        sdf::union(
            sdf::union(
                sdf::triangle(&q, &root_front, &glove_end, &root_back),
                sdf::triangle(&q, &glove_end, &tip_front, &root_back),
            ),
            sdf::triangle(&q, &tip_front, &tip_back, &root_back),
        )
    };
    let wing = sdf::round(sdf::extrude(&(m - Vector3::new(0.0, 0.0, -0.075)), planform, 0.012), 0.012);

    // Vertical tail fin: a swept quadrilateral in (x, z), extruded thinly along y.
    let fin = {
        let q = Vector2::new(p.x, p.z);
        let root_front = Vector2::new(-0.48, 0.1);
        let root_back = Vector2::new(-0.97, 0.1);
        let tip_back = Vector2::new(-1.0, 0.62);
        let tip_front = Vector2::new(-0.84, 0.62);
        let outline = sdf::union(
            sdf::triangle(&q, &root_front, &tip_front, &root_back),
            sdf::triangle(&q, &tip_front, &tip_back, &root_back),
        );
        sdf::round(sdf::extrude(&Vector3::new(p.x, p.z, p.y), outline, 0.008), 0.01)
    };

    // Orbital manoeuvring system pods, either side of the fin.
    let pods = sdf::capsule(&m, &Vector3::new(-0.5, 0.085, 0.1), &Vector3::new(-0.96, 0.085, 0.11), 0.055);

    // Body flap: a thin plate under the main engines.
    let flap = sdf::round_box(&(p - Vector3::new(-0.98, 0.0, -0.1)), &Vector3::new(0.07, 0.13, 0.015), 0.01);

    let hull = sdf::smooth_union(fuselage, wing, 0.04);
    let hull = sdf::smooth_union(hull, fin, 0.025);
    let hull = sdf::smooth_union(hull, pods, 0.03);
    let hull = sdf::union(hull, flap);

    // Three main-engine bells at the tail (one high, two low), opening toward -x and hollowed out.
    let engines = sdf::union(engine_bell(&(p - Vector3::new(-1.04, 0.0, 0.065))), engine_bell(&(m - Vector3::new(-1.04, 0.065, -0.035))));

    if engines < hull {
        return (engines, ENGINE);
    }
    (hull, hull_material(p, &m))
}

/// One engine bell centred at the origin, axis along x: narrow at +x where it meets the
/// fuselage, flaring toward -x.
fn engine_bell(p: &Vector3<f64>) -> f64 {
    // capped_cone's axis is z, with r_bottom at -z: map local -x to cone -z.
    let q = Vector3::new(p.y, p.z, p.x);
    let outer = sdf::capped_cone(&q, 0.07, 0.055, 0.025);
    let inner = sdf::capped_cone(&(q - Vector3::new(0.0, 0.0, -0.01)), 0.07, 0.048, 0.018);
    sdf::subtract(outer, inner)
}

/// Black heat-shield tiles underneath, on the nose cap and along the wing leading edges;
/// white blankets elsewhere; dark cockpit windows.
fn hull_material(p: &Vector3<f64>, m: &Vector3<f64>) -> usize {
    // Windows: a band across the front of the flight deck.
    if p.x > 0.6 && p.x < 0.7 && p.z > 0.075 && m.y < 0.075 {
        return WINDOW;
    }
    // Underside: everything below the wing's upper surface, and the nose cap.
    if p.z < -0.075 || p.x > 0.93 {
        return BLACK;
    }
    // Wing leading edges: a strip just behind the glove and main-wing front edges.
    if m.y > 0.14 && p.z < -0.05 {
        let glove = edge_distance(&Vector2::new(m.x, m.y), &Vector2::new(0.42, 0.08), &Vector2::new(-0.15, 0.26));
        let main = edge_distance(&Vector2::new(m.x, m.y), &Vector2::new(-0.15, 0.26), &Vector2::new(-0.72, 0.65));
        if glove.min(main) < 0.035 {
            return BLACK;
        }
    }
    WHITE
}

/// Distance from `p` to the segment from `a` to `b`.
fn edge_distance(p: &Vector2<f64>, a: &Vector2<f64>, b: &Vector2<f64>) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(&ab) / ab.norm_squared()).clamp(0.0, 1.0);
    (p - a - ab * t).norm()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Points on a grid over the faces of the bounding box.
    fn bounds_surface_points() -> Vec<Vector3<f64>> {
        let b = local_bounds();
        let n = 24;
        let lerp = |i: usize, lo: f64, hi: f64| lo + (hi - lo) * i as f64 / n as f64;
        let mut points = Vec::new();
        for i in 0..=n {
            for j in 0..=n {
                for axis in 0..3 {
                    for &side in &[b.min[axis], b.max[axis]] {
                        let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
                        let mut p = Vector3::zeros();
                        p[axis] = side;
                        p[a1] = lerp(i, b.min[a1], b.max[a1]);
                        p[a2] = lerp(j, b.min[a2], b.max[a2]);
                        points.push(p);
                    }
                }
            }
        }
        points
    }

    #[test]
    fn shape_fits_inside_its_bounds() {
        for p in bounds_surface_points() {
            let (d, _) = shuttle_sdf(&p);
            assert!(d > 0.005, "surface reaches the bounding box at {p:?} (d = {d})");
        }
    }

    #[test]
    fn real_proportions() {
        // Nose tip near x = 1, wing tip near y = 0.65, fin top near z = 0.62; solid inside.
        let surface_along = |dir: Vector3<f64>, from: Vector3<f64>| {
            // Walk outward until outside.
            let mut t = 0.0;
            while shuttle_sdf(&(from + dir * t)).0 < 0.0 {
                t += 1e-3;
            }
            from + dir * t
        };
        let nose = surface_along(Vector3::x(), Vector3::zeros());
        assert!((nose.x - 1.0).abs() < 0.03, "nose at {nose:?}");
        let tip = surface_along(Vector3::y(), Vector3::new(-0.82, 0.0, -0.075));
        assert!((tip.y - 0.66).abs() < 0.03, "wing tip at {tip:?}");
        let fin = surface_along(Vector3::z(), Vector3::new(-0.92, 0.0, 0.0));
        assert!((fin.z - 0.63).abs() < 0.03, "fin top at {fin:?}");
        // Symmetric.
        for p in [Vector3::new(-0.5, 0.4, -0.07), Vector3::new(0.3, 0.1, 0.05)] {
            let mirrored = Vector3::new(p.x, -p.y, p.z);
            assert_eq!(shuttle_sdf(&p), shuttle_sdf(&mirrored));
        }
    }

    #[test]
    fn distances_dont_overestimate_much() {
        // Marching relies on |d(a) - d(b)| <= |a - b| outside the shape (it never steps from
        // inside). Smooth blends and the ellipsoid bend that a little; the step scale (0.8)
        // must cover the worst case. (Inside, the ellipsoid's slope is far steeper, harmlessly.)
        let mut worst: f64 = 0.0;
        let b = local_bounds();
        let mut seed = 12345u64;
        let mut rand = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..50_000 {
            let p = Vector3::new(
                b.min.x + (b.max.x - b.min.x) * rand(),
                b.min.y + (b.max.y - b.min.y) * rand(),
                b.min.z + (b.max.z - b.min.z) * rand(),
            );
            let dir = Vector3::new(rand() - 0.5, rand() - 0.5, rand() - 0.5).normalize();
            let step = 0.01;
            let (d1, d2) = (shuttle_sdf(&p).0, shuttle_sdf(&(p + dir * step)).0);
            if d1 > 0.0 && d2 > 0.0 {
                worst = worst.max((d2 - d1).abs() / step);
            }
        }
        assert!(worst * 0.8 < 1.0, "steepest SDF slope {worst:.3}: lower the step scale");
    }
}
