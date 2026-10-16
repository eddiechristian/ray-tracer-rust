// Signed distance functions (SDFs) for ray marching, ported from Inigo Quilez's catalogue:
// https://iquilezles.org/articles/distfunctions/ (and .../distfunctions2d/ for the 2D shapes).
// Each returns the distance from `p` to the surface: positive outside, negative inside.
// Shapes are centred on the origin; the GLSL originals use y as the axis of round shapes,
// here it is z, to match this port's z-up convention.
//
// Most of these are exact distances. `ellipsoid`, `tri_prism` and the smooth operators are
// only bounds (never larger than the true distance, but possibly smaller), so marching them
// is still safe, just slower; see `RayMarched::with_step_scale`.

use nalgebra::{Vector2, Vector3};

// ---------------------------------------------------------------------------
// 3D shapes
// ---------------------------------------------------------------------------

pub fn sphere(p: &Vector3<f64>, radius: f64) -> f64 {
    p.norm() - radius
}

/// Box with half-extents `half` (iq's `sdBox`).
pub fn cuboid(p: &Vector3<f64>, half: &Vector3<f64>) -> f64 {
    let q = p.abs() - half;
    q.sup(&Vector3::zeros()).norm() + q.max().min(0.0)
}

/// Box with half-extents `half` whose edges are rounded with radius `radius`
/// (the overall size stays `half`).
pub fn round_box(p: &Vector3<f64>, half: &Vector3<f64>, radius: f64) -> f64 {
    cuboid(p, &half.add_scalar(-radius)) - radius
}

/// Line segment from `a` to `b`, thickened by `radius`.
pub fn capsule(p: &Vector3<f64>, a: &Vector3<f64>, b: &Vector3<f64>, radius: f64) -> f64 {
    let pa = p - a;
    let ba = b - a;
    let h = (pa.dot(&ba) / ba.dot(&ba)).clamp(0.0, 1.0);
    (pa - ba * h).norm() - radius
}

/// Cylinder along z: `radius`, spanning z in [-half_height, half_height].
pub fn capped_cylinder(p: &Vector3<f64>, half_height: f64, radius: f64) -> f64 {
    let d = Vector2::new(p.xy().norm(), p.z).abs() - Vector2::new(radius, half_height);
    d.max().min(0.0) + d.sup(&Vector2::zeros()).norm()
}

/// Truncated cone along z: radius `r_bottom` at z = -half_height, `r_top` at z = +half_height.
pub fn capped_cone(p: &Vector3<f64>, half_height: f64, r_bottom: f64, r_top: f64) -> f64 {
    // Revolving the 2D trapezoid about z gives the cone.
    trapezoid(&Vector2::new(p.xy().norm(), p.z), r_bottom, r_top, half_height)
}

/// Two spheres joined smoothly by a cone: radius `r1` at the origin, `r2` at z = `height`.
pub fn round_cone(p: &Vector3<f64>, r1: f64, r2: f64, height: f64) -> f64 {
    let b = (r1 - r2) / height;
    let a = (1.0 - b * b).sqrt();
    let q = Vector2::new(p.xy().norm(), p.z);
    let k = q.dot(&Vector2::new(-b, a));
    if k < 0.0 {
        q.norm() - r1
    } else if k > a * height {
        (q - Vector2::new(0.0, height)).norm() - r2
    } else {
        q.dot(&Vector2::new(a, b)) - r1
    }
}

/// Ellipsoid with semi-axes `radii` (a bound, not exact).
pub fn ellipsoid(p: &Vector3<f64>, radii: &Vector3<f64>) -> f64 {
    let k0 = p.component_div(radii).norm();
    let k1 = p.component_div(&radii.component_mul(radii)).norm();
    k0 * (k0 - 1.0) / k1
}

/// Equilateral triangular prism along z: triangle of size `size` in xy (pointing +y),
/// spanning z in [-half_length, half_length] (a bound, not exact).
pub fn tri_prism(p: &Vector3<f64>, size: f64, half_length: f64) -> f64 {
    let q = p.abs();
    (q.z - half_length).max((q.x * 0.866025 + p.y * 0.5).max(-p.y) - size * 0.5)
}

/// Ring in the xy-plane around z: `major` radius to the tube centre, tube radius `minor`.
pub fn torus(p: &Vector3<f64>, major: f64, minor: f64) -> f64 {
    Vector2::new(p.xy().norm() - major, p.z).norm() - minor
}

// ---------------------------------------------------------------------------
// 2D shapes (in the xy-plane), for `extrude`
// ---------------------------------------------------------------------------

/// Triangle with corners `a`, `b`, `c` (either winding).
pub fn triangle(p: &Vector2<f64>, a: &Vector2<f64>, b: &Vector2<f64>, c: &Vector2<f64>) -> f64 {
    let cross = |u: &Vector2<f64>, v: &Vector2<f64>| u.x * v.y - u.y * v.x;
    let edges = [(a, b - a), (b, c - b), (c, a - c)];
    let winding = cross(&edges[0].1, &edges[2].1).signum();
    let mut dist_sq = f64::INFINITY;
    let mut side = f64::INFINITY;
    for (corner, edge) in edges {
        let v = p - corner;
        // Nearest point on this edge, and which side of it `p` is on.
        let to_edge = v - edge * (v.dot(&edge) / edge.dot(&edge)).clamp(0.0, 1.0);
        dist_sq = dist_sq.min(to_edge.norm_squared());
        side = side.min(winding * cross(&v, &edge));
    }
    -dist_sq.sqrt() * side.signum()
}

/// Isosceles trapezoid symmetric about the y-axis: half-width `r_bottom` at y = -half_height,
/// `r_top` at y = +half_height.
pub fn trapezoid(p: &Vector2<f64>, r_bottom: f64, r_top: f64, half_height: f64) -> f64 {
    let k1 = Vector2::new(r_top, half_height);
    let k2 = Vector2::new(r_top - r_bottom, 2.0 * half_height);
    let p = Vector2::new(p.x.abs(), p.y);
    let cap_radius = if p.y < 0.0 { r_bottom } else { r_top };
    let ca = Vector2::new(p.x - p.x.min(cap_radius), p.y.abs() - half_height);
    let cb = p - k1 + k2 * ((k1 - p).dot(&k2) / k2.norm_squared()).clamp(0.0, 1.0);
    let sign = if cb.x < 0.0 && ca.y < 0.0 { -1.0 } else { 1.0 };
    sign * ca.norm_squared().min(cb.norm_squared()).sqrt()
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

/// Turns a 2D distance `d2` (of the shape at `p.xy()`) into a slab of it along z,
/// spanning z in [-half_depth, half_depth].
pub fn extrude(p: &Vector3<f64>, d2: f64, half_depth: f64) -> f64 {
    let w = Vector2::new(d2, p.z.abs() - half_depth);
    w.max().min(0.0) + w.sup(&Vector2::zeros()).norm()
}

/// Mirror across the xz-plane: model one side (y >= 0) and get both.
pub fn mirror_y(p: &Vector3<f64>) -> Vector3<f64> {
    Vector3::new(p.x, p.y.abs(), p.z)
}

/// Inflates a shape by `radius`, rounding its edges.
pub fn round(d: f64, radius: f64) -> f64 {
    d - radius
}

pub fn union(a: f64, b: f64) -> f64 {
    a.min(b)
}

/// `a` with `b` cut out of it.
pub fn subtract(a: f64, b: f64) -> f64 {
    a.max(-b)
}

pub fn intersect(a: f64, b: f64) -> f64 {
    a.max(b)
}

/// Union that blends the shapes together over a distance of about `k`.
pub fn smooth_union(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    lerp(b, a, h) - k * h * (1.0 - h)
}

/// `a` with `b` cut out of it, with the cut edge rounded over about `k`.
pub fn smooth_subtract(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 - 0.5 * (a + b) / k).clamp(0.0, 1.0);
    lerp(a, -b, h) + k * h * (1.0 - h)
}

/// Intersection with the edge where the shapes meet rounded over about `k`.
pub fn smooth_intersect(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 - 0.5 * (b - a) / k).clamp(0.0, 1.0);
    lerp(b, a, h) + k * h * (1.0 - h)
}

/// Union of two (distance, material index) pairs: the nearer shape's material wins.
pub fn union_id(a: (f64, usize), b: (f64, usize)) -> (f64, usize) {
    if a.0 <= b.0 { a } else { b }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v3(x: f64, y: f64, z: f64) -> Vector3<f64> {
        Vector3::new(x, y, z)
    }

    fn v2(x: f64, y: f64) -> Vector2<f64> {
        Vector2::new(x, y)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // A few points inside, on, and outside, along axes and diagonals.
    fn sample_points() -> Vec<Vector3<f64>> {
        let mut points = vec![v3(0.0, 0.0, 0.0)];
        for &s in &[0.3, 1.0, 2.5] {
            points.extend([v3(s, 0.0, 0.0), v3(0.0, -s, 0.0), v3(0.0, 0.0, s), v3(s, s, -s), v3(-s, 0.5 * s, 2.0 * s)]);
        }
        points
    }

    #[test]
    fn sphere_and_box_match_exact_distances() {
        assert!(close(sphere(&v3(3.0, 0.0, 0.0), 1.0), 2.0));
        assert!(close(sphere(&v3(0.0, 0.0, 0.0), 1.0), -1.0));

        let half = Vector3::repeat(1.0);
        assert!(close(cuboid(&v3(2.0, 0.0, 0.0), &half), 1.0)); // off a face
        assert!(close(cuboid(&v3(2.0, 2.0, 2.0), &half), 3f64.sqrt())); // off a corner
        assert!(close(cuboid(&v3(0.0, 0.0, 0.0), &half), -1.0)); // centre
        assert!(close(cuboid(&v3(0.5, 0.0, 0.0), &half), -0.5));

        // Rounded box: the corner is a sphere of radius 0.5 centred at (0.5, 0.5, 0.5).
        assert!(close(round_box(&v3(2.0, 2.0, 2.0), &half, 0.5), (3.0 * 1.5f64.powi(2)).sqrt() - 0.5));
        assert!(close(round_box(&v3(2.0, 0.0, 0.0), &half, 0.5), 1.0)); // faces don't move
    }

    #[test]
    fn round_shapes_match_their_special_cases() {
        for p in sample_points() {
            // A cylinder: on axis above the cap, and beside the side.
            assert!(close(capped_cylinder(&v3(0.0, 0.0, 3.0), 1.0, 1.0), 2.0));
            assert!(close(capped_cylinder(&v3(3.0, 0.0, 0.0), 1.0, 1.0), 2.0));
            // A cone with equal radii is a cylinder.
            assert!(close(capped_cone(&p, 1.0, 0.7, 0.7), capped_cylinder(&p, 1.0, 0.7)), "{p:?}");
            // A round cone with equal radii is a capsule.
            let capsule_d = capsule(&p, &v3(0.0, 0.0, 0.0), &v3(0.0, 0.0, 2.0), 0.5);
            assert!(close(round_cone(&p, 0.5, 0.5, 2.0), capsule_d), "{p:?}");
            // An ellipsoid with equal radii is a sphere (away from the centre, where it's 0/0).
            if p.norm() > 0.0 {
                assert!(close(ellipsoid(&p, &Vector3::repeat(1.5)), sphere(&p, 1.5)), "{p:?}");
            }
        }
        assert!(close(torus(&v3(2.0, 0.0, 0.0), 2.0, 0.5), -0.5)); // inside the tube
        assert!(close(torus(&v3(0.0, 0.0, 0.0), 2.0, 0.5), 1.5)); // the hole
        assert!(close(torus(&v3(0.0, 2.0, 1.5), 2.0, 0.5), 1.0)); // above the tube
    }

    #[test]
    fn cone_tapers() {
        // Radius 1 at the bottom, 0 at the top: halfway up the side is at radius 0.5.
        assert!(capped_cone(&v3(0.5, 0.0, 0.0), 1.0, 1.0, 0.0).abs() < 1e-9);
        assert!(capped_cone(&v3(0.4, 0.0, 0.0), 1.0, 1.0, 0.0) < 0.0);
        assert!(capped_cone(&v3(0.6, 0.0, 0.0), 1.0, 1.0, 0.0) > 0.0);
        // Below the bottom cap.
        assert!(close(capped_cone(&v3(0.0, 0.0, -3.0), 1.0, 1.0, 0.0), 2.0));
    }

    #[test]
    fn two_d_shapes() {
        let (a, b, c) = (v2(-1.0, 0.0), v2(1.0, 0.0), v2(0.0, 1.0));
        assert!(close(triangle(&v2(0.0, -1.0), &a, &b, &c), 1.0)); // below the base
        assert!(close(triangle(&v2(3.0, 0.0), &a, &b, &c), 2.0)); // off a corner
        assert!(close(triangle(&v2(0.0, 0.25), &a, &b, &c), -0.25)); // inside, nearest the base
        // Winding doesn't matter.
        assert!(close(triangle(&v2(0.0, 0.25), &a, &c, &b), -0.25));

        // A trapezoid with equal widths is a rectangle.
        for p in [v2(0.0, 0.0), v2(2.0, 0.0), v2(0.0, -3.0), v2(2.0, 2.0), v2(0.3, 0.4)] {
            let rect = Vector3::new(p.x, p.y, 0.0);
            assert!(close(trapezoid(&p, 1.0, 1.0, 1.0), cuboid(&rect, &v3(1.0, 1.0, 1.0))), "{p:?}");
        }
    }

    #[test]
    fn extruded_square_is_a_box() {
        for p in sample_points() {
            let square = trapezoid(&p.xy(), 1.0, 1.0, 0.5);
            let slab = extrude(&p, square, 0.25);
            assert!(close(slab, cuboid(&p, &v3(1.0, 0.5, 0.25))), "{p:?}");
        }
        assert!(close(extrude(&v3(0.0, 0.25, 2.0), triangle(&v2(0.0, 0.25), &v2(-1.0, 0.0), &v2(1.0, 0.0), &v2(0.0, 1.0)), 1.0), 1.0));
    }

    #[test]
    fn operators() {
        assert_eq!(union(1.0, 2.0), 1.0);
        assert_eq!(subtract(1.0, -2.0), 2.0); // deep inside the cutter: outside the result
        assert_eq!(intersect(1.0, 2.0), 2.0);
        assert_eq!(mirror_y(&v3(1.0, -2.0, 3.0)), v3(1.0, 2.0, 3.0));
        assert_eq!(union_id((1.0, 0), (0.5, 1)), (0.5, 1));
        // Smooth versions equal the sharp ones when the shapes are far apart (> k)...
        assert!(close(smooth_union(1.0, 3.0, 0.5), 1.0));
        assert!(close(smooth_subtract(-1.0, -3.0, 0.5), subtract(-1.0, -3.0)));
        assert!(close(smooth_intersect(1.0, 3.0, 0.5), 3.0));
        // ...and add material in the blend region where they're close.
        assert!(smooth_union(1.0, 1.0, 0.5) < 1.0);
    }
}
