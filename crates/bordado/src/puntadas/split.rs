//! Cuts: segments across an area that split it in parts, each one sewn as its own column.
//! Letters with branches (H, T, E, K) aren't one column: they are cut where the strokes meet.
//!
//! A cut with both ends on the outer edge splits the part in two. A cut from the outer edge to
//! a hole (or between two holes) opens the hole: both rings become one, joined along the cut
//! (a slit of no width), so an «e» or an «8» can be sewn as a column that goes around.
//!
//! [`auto_cuts`] finds the cuts at the corners where strokes meet: corners of the edge that
//! turn away from the area.

use kurbo::{Point, Vec2};

use super::column::{Guide, Ring, area, components, inside_ring, owner};

/// How far (mm) the ends of a cut may be from the edges.
const SNAP: f64 = 1.0;
/// A vertex turning more than this away from the area is a corner where two strokes meet.
const CORNER_DEG: f64 = 40.0;
/// How far (mm) a cut must stay from the edges, except at its ends.
const CLEARANCE: f64 = 0.02;
const EPS: f64 = 1e-6;

/// The parts of the area (even-odd rings) once cut: each one an outer ring followed by its
/// holes, which go the other way round.
pub(super) fn split(rings: &[Vec<Point>], cuts: &[Guide]) -> Vec<Vec<Ring>> {
    let mut parts = oriented(components(rings));
    for cut in cuts {
        apply(&mut parts, cut);
    }
    parts
}

/// Cuts at the corners where strokes meet. Two corners facing each other across a stroke are
/// joined (the bar of an H, under the bar of a T); a corner left alone is cut along one of its
/// edges, continued across the stroke (the arms of an E, an L).
pub fn auto_cuts(rings: &[Vec<Point>]) -> Vec<Guide> {
    let mut out: Vec<Guide> = vec![];
    for part in oriented(components(rings)) {
        let sign = area(&part[0].pts).signum();
        let corners: Vec<Corner> = part.iter().flat_map(|r| corners(r, sign)).collect();
        let mut pairs: Vec<(f64, usize, usize)> = vec![];
        for i in 0..corners.len() {
            for j in i + 1..corners.len() {
                let (a, b) = (corners[i].p, corners[j].p);
                if inside(&part, a, b) {
                    pairs.push((a.distance(b), i, j));
                }
            }
        }
        pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut used = vec![false; corners.len()];
        let first = out.len();
        for (_, i, j) in pairs {
            let cut = [corners[i].p, corners[j].p];
            if !used[i] && !used[j] && !out[first..].iter().any(|c| meet(c, &cut)) {
                used[i] = true;
                used[j] = true;
                out.push(cut);
            }
        }
        for (c, _) in corners.iter().zip(&used).filter(|(_, u)| !**u) {
            let shortest = [c.inn, -c.out]
                .into_iter()
                .filter_map(|d| {
                    let t = ray(&part, c.p, d)?;
                    let cut = [c.p, c.p + d * t];
                    (inside(&part, cut[0], cut[1]) && !out[first..].iter().any(|o| meet(o, &cut)))
                        .then_some((t, cut))
                })
                .min_by(|x, y| x.0.total_cmp(&y.0));
            if let Some((_, cut)) = shortest {
                out.push(cut);
            }
        }
    }
    out
}

/// Holes walked the other way round from their outer ring.
fn oriented(parts: Vec<Vec<Ring>>) -> Vec<Vec<Ring>> {
    parts
        .into_iter()
        .map(|mut part| {
            let sign = area(&part[0].pts).signum();
            for hole in &mut part[1..] {
                if area(&hole.pts).signum() == sign {
                    let mut pts = hole.pts.clone();
                    pts.reverse();
                    *hole = Ring::new(&pts);
                }
            }
            part
        })
        .collect()
}

/// Applies one cut to the part it falls on. Cuts that don't go across a part are ignored.
fn apply(parts: &mut Vec<Vec<Ring>>, cut: &Guide) {
    let Some(k) = owner(parts, cut) else {
        return;
    };
    let part = &parts[k];
    // Which ring each end is on, and where.
    let on = |p: Point| {
        part.iter()
            .enumerate()
            .map(|(i, r)| {
                let (d, s) = r.project(p);
                (d, i, s)
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .unwrap()
    };
    let ((d0, i0, s0), (d1, i1, s1)) = (on(cut[0]), on(cut[1]));
    let (a, b) = (part[i0].at(s0), part[i1].at(s1));
    if d0 > SNAP || d1 > SNAP || !inside(part, a, b) {
        return;
    }
    if i0 == i1 {
        if i0 != 0 {
            // Both ends on the same hole: it doesn't cut anything.
            return;
        }
        let r = &part[0];
        let (l0, l1) = (r.fwd(s0, s1), r.fwd(s1, s0));
        let sides = [r.walk(s0, l0), r.walk(s1, l1)];
        if sides.iter().any(|s| s.len() < 3 || area(s).abs() < EPS) {
            return;
        }
        let mut new: Vec<Vec<Ring>> = sides.iter().map(|s| vec![Ring::new(s)]).collect();
        for hole in &part[1..] {
            let side = if inside_ring(&sides[0], hole.pts[0]) {
                0
            } else {
                1
            };
            new[side].push(Ring::new(&hole.pts));
        }
        let second = new.pop().unwrap();
        parts[k] = new.pop().unwrap();
        parts.insert(k + 1, second);
    } else {
        // Two rings joined along the cut: around the first one, across, around the second one
        // and back across.
        let (r0, r1) = (&part[i0], &part[i1]);
        let mut pts = r0.walk(s0, r0.len());
        pts.extend(r1.walk(s1, r1.len()));
        let joined = Ring::new(&pts);
        let part = &mut parts[k];
        let (lo, hi) = (i0.min(i1), i0.max(i1));
        part.remove(hi);
        part[lo] = joined;
    }
}

/// A corner where two strokes meet: the vertex, and the edge directions arriving and leaving.
struct Corner {
    p: Point,
    inn: Vec2,
    out: Vec2,
}

/// The vertices of the ring that turn away from the area (`sign` = the outer ring's
/// orientation) by more than [`CORNER_DEG`].
fn corners(ring: &Ring, sign: f64) -> Vec<Corner> {
    let pts = &ring.pts;
    let n = pts.len();
    // The nearest point that isn't the same one, going `step` (±1) from `i`.
    let other = |i: usize, step: isize| {
        (1..n)
            .map(|k| pts[(i as isize + step * k as isize).rem_euclid(n as isize) as usize])
            .find(|q| q.distance(pts[i]) > EPS)
    };
    (0..n)
        .filter_map(|i| {
            let (prev, next) = (other(i, -1)?, other(i, 1)?);
            let (inn, out) = ((pts[i] - prev).normalize(), (next - pts[i]).normalize());
            let turn = inn.cross(out).atan2(inn.dot(out));
            (turn * sign < -CORNER_DEG.to_radians()).then_some(Corner {
                p: pts[i],
                inn,
                out,
            })
        })
        .collect()
}

/// Whether segment `a`–`b` goes through the inside of the part (touching the edges only at its
/// ends).
fn inside(part: &[Ring], a: Point, b: Point) -> bool {
    if a.distance(b) < 0.1 {
        return false;
    }
    let crosses = part.iter().any(|r| {
        let n = r.pts.len();
        (0..n).any(|i| meet(&[a, b], &[r.pts[i], r.pts[(i + 1) % n]]))
    });
    let clear = |p: Point| {
        part.iter().all(|r| r.project(p).0 > CLEARANCE)
            && part.iter().filter(|r| inside_ring(&r.pts, p)).count() % 2 == 1
    };
    !crosses && [0.25, 0.5, 0.75].iter().all(|&t| clear(a.lerp(b, t)))
}

/// Whether two segments cross (touching at an end doesn't count).
fn meet(p: &Guide, q: &Guide) -> bool {
    let side = |a: Point, b: Point, c: Point| (b - a).cross(c - a);
    let (d1, d2) = (side(q[0], q[1], p[0]), side(q[0], q[1], p[1]));
    let (d3, d4) = (side(p[0], p[1], q[0]), side(p[0], p[1], q[1]));
    d1 * d2 < -1e-9 && d3 * d4 < -1e-9
}

/// How far the ray from `p` along `d` (unit) goes before it hits an edge of the part.
fn ray(part: &[Ring], p: Point, d: Vec2) -> Option<f64> {
    part.iter()
        .flat_map(|r| {
            let n = r.pts.len();
            (0..n).filter_map(move |i| {
                let (a, b) = (r.pts[i], r.pts[(i + 1) % n]);
                let e = b - a;
                let den = d.cross(e);
                if den.abs() < 1e-12 {
                    return None;
                }
                let t = (a - p).cross(e) / den;
                let u = (a - p).cross(d) / den;
                (t > 1e-6 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some(t)
            })
        })
        .min_by(f64::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::polilineas;

    fn rings(d: &str) -> Vec<Vec<Point>> {
        polilineas(&kurbo::BezPath::from_svg(d).unwrap())
            .into_iter()
            .map(|pl| pl.puntos)
            .collect()
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Whether `c` is the segment `a`–`b`, either way round.
    fn is(c: &Guide, a: Point, b: Point) -> bool {
        let close = |x: Point, y: Point| x.distance(y) < 1e-6;
        (close(c[0], a) && close(c[1], b)) || (close(c[0], b) && close(c[1], a))
    }

    const H: &str = "M0,0 L3,0 L3,8 L9,8 L9,0 L12,0 L12,20 L9,20 L9,11 L3,11 L3,20 L0,20 Z";
    const T: &str = "M0,0 L14,0 L14,3 L8.5,3 L8.5,20 L5.5,20 L5.5,3 L0,3 Z";
    const E: &str = "M0,0 L12,0 L12,3 L3,3 L3,8.5 L10,8.5 L10,11.5 L3,11.5 L3,17 L12,17 \
                     L12,20 L0,20 Z";

    #[test]
    fn a_cut_across_splits_in_two() {
        let d = "M0,0 L10,0 L10,4 L0,4 Z";
        let parts = split(&rings(d), &[[p(4.0, -0.3), p(4.0, 4.2)]]);
        assert_eq!(parts.len(), 2);
        let areas: Vec<f64> = parts.iter().map(|q| area(&q[0].pts).abs()).collect();
        assert!((areas[0] + areas[1] - 40.0).abs() < 1e-6, "{areas:?}");
        assert!((areas[0] - 16.0).abs() < 1e-6 || (areas[1] - 16.0).abs() < 1e-6);
    }

    #[test]
    fn a_cut_to_the_hole_opens_it() {
        let d = "M0,0 L10,0 L10,10 L0,10 Z M3,3 L7,3 L7,7 L3,7 Z";
        let parts = split(&rings(d), &[[p(5.0, 0.0), p(5.0, 3.0)]]);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].len(), 1, "no holes left");
        assert!((area(&parts[0][0].pts).abs() - 84.0).abs() < 1e-6);
    }

    #[test]
    fn cuts_outside_or_along_the_edge_are_ignored() {
        let d = "M0,0 L10,0 L10,4 L0,4 Z";
        assert_eq!(split(&rings(d), &[[p(0.0, -3.0), p(10.0, -3.0)]]).len(), 1);
        assert_eq!(split(&rings(d), &[[p(2.0, 0.0), p(8.0, 0.0)]]).len(), 1);
    }

    #[test]
    fn auto_cuts_where_strokes_meet() {
        let h = auto_cuts(&rings(H));
        assert_eq!(h.len(), 2, "{h:?}");
        assert!(h.iter().any(|c| is(c, p(3.0, 8.0), p(3.0, 11.0))), "{h:?}");
        assert!(h.iter().any(|c| is(c, p(9.0, 8.0), p(9.0, 11.0))), "{h:?}");
        assert_eq!(split(&rings(H), &h).len(), 3);

        let t = auto_cuts(&rings(T));
        assert_eq!(t.len(), 1, "{t:?}");
        assert!(is(&t[0], p(5.5, 3.0), p(8.5, 3.0)), "{t:?}");

        // The middle arm by its two corners; the top and bottom ones along the stem.
        let e = auto_cuts(&rings(E));
        assert_eq!(e.len(), 3, "{e:?}");
        assert!(e.iter().any(|c| is(c, p(3.0, 8.5), p(3.0, 11.5))), "{e:?}");
        assert_eq!(split(&rings(E), &e).len(), 4);
    }

    #[test]
    fn round_letters_have_no_cuts() {
        let c = "M10,0 A10,10 0 1 0 10,20 L10,16 A6,6 0 1 1 10,4 Z";
        let o = "M-10,0 A10,10 0 1 1 10,0 A10,10 0 1 1 -10,0 Z \
                 M-7,0 A7,7 0 1 0 7,0 A7,7 0 1 0 -7,0 Z";
        assert!(auto_cuts(&rings(c)).is_empty());
        assert!(auto_cuts(&rings(o)).is_empty());
    }
}
