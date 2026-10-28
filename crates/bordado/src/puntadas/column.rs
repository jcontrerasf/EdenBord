//! Satin that fills a closed area and turns with it.
//!
//! The column runs between two edges of the shape (the rails). Angle guides are segments that
//! cross the shape from one rail to the other: between two guides the stitches are interpolated
//! by arc length on both rails, so they turn from one guide's angle to the next. Guides can be
//! drawn by hand or computed from the shape ([`auto_guides`]).
//!
//! A part with no holes (C, l, s) is a column between two ends: the first and last guides.
//! A part with one hole (o, e) is a ring: the outer edge and the hole are the rails. Parts
//! with more holes can't be sewn as one column. Letters with branches (H, T, E) are first split
//! in parts by cuts ([`super::split`]).

use kurbo::Point;

use crate::geom::{Arco, unitario};
use crate::proyecto::BaseSatin;

use super::Tramo;
use super::satin::sew;
use super::split::split;

/// A guide: a segment across the column, from one rail to the other (mm).
pub type Guide = [Point; 2];

/// Satin stitches longer than this are split in staggered pieces so they don't snag.
const MAX_STITCH: f64 = 7.0;
/// How far (mm) the edge may stray from a chord and still count as straight, and the shortest
/// straight stretch that can be a cut.
const STRAIGHT_TOL: f64 = 0.05;
const MIN_STRETCH: f64 = 0.3;
/// A straight stretch is a cut across the column if it is within ~45° of a rung near the tip.
const CUT_COS: f64 = 0.7;
/// How far (mm) to look past a straight stretch to see whether the edge turns there (a flat
/// end), and how much wider two flat ends may be than two tips and still be the ends.
const FLAT_PROBE: f64 = 0.5;
const FLAT_BIAS: f64 = 1.5;
/// Longest cut, in widths of the column measured a little into it (the tip may be narrower).
const MAX_CUT: f64 = 2.0;
/// Candidate positions per ring when looking for the two ends of a column.
const END_CANDIDATES: usize = 48;
/// Samples per rail when comparing candidate ends.
const END_SAMPLES: usize = 32;
/// Most samples per ring for the dense matching of the rails, and their smallest spacing.
const MATCH_SAMPLES: f64 = 400.0;
const MATCH_STEP: f64 = 0.3;
/// How far (mm, on each rail) the simplified guides may stray from the dense matching.
const SIMPLIFY_TOL: f64 = 0.4;
/// Longest stretch (mm, on the longer rail) between automatic guides, so there are always
/// some to grab along the column.
const MAX_GAP: f64 = 10.0;
const EPS: f64 = 1e-6;

/// Stitches of an area satin.
pub struct AreaSatin {
    pub runs: Vec<Tramo>,
    /// Parts that couldn't be sewn as a column (more than one hole).
    pub skipped: usize,
}

/// Satin column filling each part of the area (even-odd rings, split by `cuts`), turning with
/// the guides: `None` = automatic; with guides of its own, a part with fewer than two gets the
/// automatic ends. The columns are sewn starting with
/// the one closest to the needle (at `from`), each from its closest end.
pub fn area_satin(
    rings: &[Vec<Point>],
    guides: Option<&[Guide]>,
    cuts: &[Guide],
    density: f64,
    base: BaseSatin,
    compensation: f64,
    from: Point,
) -> AreaSatin {
    let all = columns(rings, guides, cuts, density);
    let skipped = all.iter().filter(|c| c.is_none()).count();
    let mut left: Vec<Vec<(Point, Point)>> =
        all.into_iter().flatten().filter(|c| c.len() > 1).collect();
    let mut out = AreaSatin {
        runs: vec![],
        skipped,
    };
    let mut at = from;
    while !left.is_empty() {
        let mid = |(a, b): (Point, Point)| a.midpoint(b).distance(at);
        let (k, reverse) = left
            .iter()
            .enumerate()
            .flat_map(|(k, c)| [(k, false, mid(c[0])), (k, true, mid(c[c.len() - 1]))])
            .min_by(|x, y| x.2.total_cmp(&y.2))
            .map(|(k, r, _)| (k, r))
            .unwrap();
        let mut pairs = left.swap_remove(k);
        if reverse {
            pairs.reverse();
        }
        let pairs: Vec<_> = pairs
            .into_iter()
            .map(|(a, b)| {
                let d = unitario(a - b) * compensation;
                (a + d, b - d)
            })
            .collect();
        let run = split_long(&sew(&pairs, density, base), MAX_STITCH);
        at = run.last().copied().unwrap_or(at);
        out.runs.push(run);
    }
    out
}

/// The (left, right) edge pairs of the column of each part (split by `cuts`), `density / 2`
/// apart (measured on the longer rail); `None` for the parts that can't be a column. Guides as
/// in [`area_satin`].
pub fn columns(
    rings: &[Vec<Point>],
    guides: Option<&[Guide]>,
    cuts: &[Guide],
    density: f64,
) -> Vec<Option<Vec<(Point, Point)>>> {
    let parts = split(rings, cuts);
    let mut own: Vec<Vec<Guide>> = vec![vec![]; parts.len()];
    for g in guides.unwrap_or_default() {
        if let Some(k) = owner(&parts, g) {
            own[k].push(*g);
        }
    }
    parts
        .iter()
        .zip(&own)
        .map(|(part, own)| {
            let c = column(part, guides.is_some().then_some(&own[..]))?;
            Some(pairs(&c, density / 2.0))
        })
        .collect()
}

/// Guides computed from the shape (split by `cuts`): the ends of each column and as many in
/// between as needed to follow it.
pub fn auto_guides(rings: &[Vec<Point>], cuts: &[Guide]) -> Vec<Guide> {
    split(rings, cuts)
        .iter()
        .flat_map(|part| match &part[..] {
            [ring] => auto_single(ring),
            [outer, hole] => auto_ring(outer, &oriented_like(hole, outer)),
            _ => vec![],
        })
        .collect()
}

/// A closed ring measured by arc length (including the closing segment).
pub(super) struct Ring {
    pub(super) pts: Vec<Point>,
    /// Length up to each point; one more entry than points (the whole ring).
    cum: Vec<f64>,
}

impl Ring {
    pub(super) fn new(pts: &[Point]) -> Self {
        let n = pts.len();
        let mut cum = Vec::with_capacity(n + 1);
        let mut total = 0.0;
        cum.push(0.0);
        for i in 0..n {
            total += pts[i].distance(pts[(i + 1) % n]);
            cum.push(total);
        }
        Ring {
            pts: pts.to_vec(),
            cum,
        }
    }

    pub(super) fn len(&self) -> f64 {
        *self.cum.last().unwrap()
    }

    /// Distance going forward from `from` to `to`.
    pub(super) fn fwd(&self, from: f64, to: f64) -> f64 {
        (to - from).rem_euclid(self.len())
    }

    pub(super) fn at(&self, s: f64) -> Point {
        let n = self.pts.len();
        let s = s.rem_euclid(self.len());
        let i = self.cum[1..].partition_point(|&c| c < s).min(n - 1);
        let l = self.cum[i + 1] - self.cum[i];
        let t = if l > 0.0 { (s - self.cum[i]) / l } else { 0.0 };
        self.pts[i].lerp(self.pts[(i + 1) % n], t)
    }

    /// Distance to the nearest point of the ring and its position.
    pub(super) fn project(&self, p: Point) -> (f64, f64) {
        let n = self.pts.len();
        let mut best = (f64::MAX, 0.0);
        for i in 0..n {
            let (a, b) = (self.pts[i], self.pts[(i + 1) % n]);
            let ab = b - a;
            let l2 = ab.hypot2();
            let t = if l2 > 0.0 {
                ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = p.distance(a + ab * t);
            if d < best.0 {
                best = (d, self.cum[i] + t * (self.cum[i + 1] - self.cum[i]));
            }
        }
        best
    }

    /// The ring walked forward from `from` for `dist` mm, both ends included.
    pub(super) fn walk(&self, from: f64, dist: f64) -> Vec<Point> {
        let n = self.pts.len();
        let start = from.rem_euclid(self.len());
        let mut out = vec![self.at(start)];
        for j in 0..3 * n {
            let s = self.cum[j % n] + (j / n) as f64 * self.len();
            if s > start + EPS && s < start + dist - EPS {
                out.push(self.pts[j % n]);
            }
        }
        out.push(self.at(start + dist));
        out
    }

    /// The ring walked backward from `from` for `dist` mm.
    fn walk_back(&self, from: f64, dist: f64) -> Vec<Point> {
        let mut v = self.walk(from - dist, dist);
        v.reverse();
        v
    }
}

/// A column ready to sew: both rails and the matching positions on them (from the first end
/// to the last, both included).
struct Column {
    a: Arco,
    b: Arco,
    rungs: Vec<(f64, f64)>,
}

/// Groups the rings by even-odd: each outer ring followed by its holes.
pub(super) fn components(rings: &[Vec<Point>]) -> Vec<Vec<Ring>> {
    let rings: Vec<&Vec<Point>> = rings.iter().filter(|r| r.len() >= 3).collect();
    let depth: Vec<usize> = (0..rings.len())
        .map(|i| {
            (0..rings.len())
                .filter(|&j| j != i && inside_ring(rings[j], rings[i][0]))
                .count()
        })
        .collect();
    let outers: Vec<usize> = (0..rings.len())
        .filter(|&i| depth[i].is_multiple_of(2))
        .collect();
    let mut parts: Vec<Vec<Ring>> = outers.iter().map(|&i| vec![Ring::new(rings[i])]).collect();
    for i in (0..rings.len()).filter(|&i| !depth[i].is_multiple_of(2)) {
        let parent = outers
            .iter()
            .enumerate()
            .filter(|&(_, &o)| depth[o] + 1 == depth[i] && inside_ring(rings[o], rings[i][0]))
            .min_by(|a, b| area(rings[*a.1]).abs().total_cmp(&area(rings[*b.1]).abs()));
        if let Some((k, _)) = parent {
            parts[k].push(Ring::new(rings[i]));
        }
    }
    parts
}

/// The part whose edges are closest to both ends of the guide.
pub(super) fn owner(parts: &[Vec<Ring>], g: &Guide) -> Option<usize> {
    let dist =
        |part: &Vec<Ring>, p: Point| part.iter().map(|r| r.project(p).0).fold(f64::MAX, f64::min);
    (0..parts.len()).min_by(|&i, &j| {
        let d = |k: usize| dist(&parts[k], g[0]) + dist(&parts[k], g[1]);
        d(i).total_cmp(&d(j))
    })
}

fn column(part: &[Ring], guides: Option<&[Guide]>) -> Option<Column> {
    match part {
        [ring] => single(ring, guides, true),
        [outer, hole] => ring_column(outer, &oriented_like(hole, outer), guides),
        _ => None,
    }
}

/// Column of a part without holes. The ends are the two guides that have all the others on one
/// side; `None`: the automatic ones; with fewer than two, the automatic ends.
fn single(ring: &Ring, guides: Option<&[Guide]>, retry: bool) -> Option<Column> {
    let mut guides = guides.map_or_else(|| auto_single(ring), <[Guide]>::to_vec);
    if guides.len() < 2 {
        let auto = auto_single(ring);
        let (first, last) = (*auto.first()?, *auto.last()?);
        guides.insert(0, first);
        guides.push(last);
    }
    let params: Vec<(f64, f64)> = guides
        .iter()
        .map(|g| (ring.project(g[0]).1, ring.project(g[1]).1))
        .collect();
    let Some((e0, e1)) = ends(ring, &params) else {
        // Crossing guides: start over from the automatic ones.
        return if retry {
            single(ring, None, false)
        } else {
            None
        };
    };
    let (p0, p1) = (params[e0], params[e1]);
    // Going forward from a0 the ends come in this order: a0, an, bn, b0.
    let ordered = |a0: f64, b0: f64, an: f64, bn: f64| {
        let f0 = ring.fwd(a0, b0);
        let f0 = if f0 < EPS { ring.len() } else { f0 };
        ring.fwd(a0, an) <= ring.fwd(a0, bn) + EPS && ring.fwd(a0, bn) <= f0 + EPS
    };
    let (a0, b0, an, bn) = [
        (p0.0, p0.1, p1.0, p1.1),
        (p0.0, p0.1, p1.1, p1.0),
        (p0.1, p0.0, p1.0, p1.1),
        (p0.1, p0.0, p1.1, p1.0),
    ]
    .into_iter()
    .find(|&(a0, b0, an, bn)| ordered(a0, b0, an, bn))?;
    let (la, lb) = (ring.fwd(a0, an), ring.fwd(bn, b0));
    let on_a = |s: f64| (ring.fwd(a0, s) <= la + EPS).then(|| ring.fwd(a0, s));
    let on_b = |s: f64| (ring.fwd(bn, s) <= lb + EPS).then(|| ring.fwd(s, b0));
    let middle: Vec<(f64, f64)> = params
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != e0 && i != e1)
        .filter_map(|(_, &(p, q))| match (on_a(p), on_b(q), on_a(q), on_b(p)) {
            (Some(sa), Some(sb), _, _) | (_, _, Some(sa), Some(sb)) => Some((sa, sb)),
            _ => None,
        })
        .collect();
    Some(Column {
        a: Arco::new(&ring.walk(a0, la)),
        b: Arco::new(&ring.walk_back(b0, lb)),
        rungs: monotonic(middle, (la, lb)),
    })
}

/// Column of a part with one hole: around the ring, from the first guide back to it (`None` or
/// none: the automatic ones, or only where they start).
fn ring_column(outer: &Ring, hole: &Ring, guides: Option<&[Guide]>) -> Option<Column> {
    let mut auto = vec![];
    let guides = match guides {
        Some(g) if !g.is_empty() => g,
        _ => {
            auto = auto_ring(outer, hole);
            if guides.is_some() {
                auto.truncate(1);
            }
            &auto[..]
        }
    };
    let params: Vec<(f64, f64)> = guides
        .iter()
        .map(|g| {
            let (o0, h1) = (outer.project(g[0]), hole.project(g[1]));
            let (o1, h0) = (outer.project(g[1]), hole.project(g[0]));
            if o0.0 + h1.0 <= o1.0 + h0.0 {
                (o0.1, h1.1)
            } else {
                (o1.1, h0.1)
            }
        })
        .collect();
    let &(so, sh) = params.first()?;
    let (la, lb) = (outer.len(), hole.len());
    let middle = params[1..]
        .iter()
        .map(|&(o, h)| (outer.fwd(so, o), hole.fwd(sh, h)))
        .collect();
    Some(Column {
        a: Arco::new(&outer.walk(so, la)),
        b: Arco::new(&hole.walk(sh, lb)),
        rungs: monotonic(middle, (la, lb)),
    })
}

/// The two end guides: the ones with every other guide on the same side. `None` if there
/// aren't two.
fn ends(ring: &Ring, params: &[(f64, f64)]) -> Option<(usize, usize)> {
    let is_end = |i: usize| {
        let (u, v) = params[i];
        let span = ring.fwd(u, v);
        let side = |s: f64| ring.fwd(u, s) < span - EPS && ring.fwd(u, s) > EPS;
        let mut sides = params
            .iter()
            .enumerate()
            .filter(|&(j, _)| j != i)
            .map(|(_, &(p, q))| (side(p), side(q)));
        let Some(first) = sides.next() else {
            return true;
        };
        sides.all(|s| s == first) && (first.0 == first.1 || span < EPS)
    };
    let candidates: Vec<usize> = (0..params.len()).filter(|&i| is_end(i)).collect();
    match candidates[..] {
        [] | [_] => None,
        [first, .., last] => Some((first, last)),
    }
}

/// Sorts the guides along the column and drops those that would cross the previous ones.
/// Adds both ends: `(0, 0)` and `last`.
fn monotonic(mut middle: Vec<(f64, f64)>, last: (f64, f64)) -> Vec<(f64, f64)> {
    middle.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out = vec![(0.0, 0.0)];
    for (sa, sb) in middle {
        let &(pa, pb) = out.last().unwrap();
        let advances = sa > pa + EPS || sb > pb + EPS;
        if advances && sa >= pa && sb >= pb && sa <= last.0 && sb <= last.1 {
            out.push((sa, sb));
        }
    }
    out.push(last);
    out
}

/// Edge pairs between consecutive rungs, interpolated by arc length; `step` is the spacing on
/// the longer rail.
fn pairs(c: &Column, step: f64) -> Vec<(Point, Point)> {
    let mut out = Vec::new();
    for w in c.rungs.windows(2) {
        let ((a0, b0), (a1, b1)) = (w[0], w[1]);
        let n = ((a1 - a0).max(b1 - b0) / step).ceil().max(1.0) as usize;
        for k in 0..n {
            let u = k as f64 / n as f64;
            out.push((c.a.en(a0 + (a1 - a0) * u).0, c.b.en(b0 + (b1 - b0) * u).0));
        }
    }
    if let Some(&(a, b)) = c.rungs.last() {
        out.push((c.a.en(a).0, c.b.en(b).0));
    }
    out
}

/// Splits the segments longer than `max` in equal pieces; every other one shifted half a
/// piece, so the needle holes don't line up.
fn split_long(points: &[Point], max: f64) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    let mut long = 0;
    for &p in points {
        if let Some(&q) = out.last() {
            let n = (q.distance(p) / max).ceil() as usize;
            if n > 1 {
                long += 1;
                let shift = if long % 2 == 0 { 0.5 } else { 0.0 };
                for k in 1..n {
                    out.push(q.lerp(p, (k as f64 - shift) / n as f64));
                }
                if shift > 0.0 {
                    out.push(q.lerp(p, (n as f64 - shift) / n as f64));
                }
            }
        }
        out.push(p);
    }
    out
}

/// Automatic guides of a part without holes: finds the two tips, matches both rails and keeps
/// the guides needed to follow the matching.
fn auto_single(ring: &Ring) -> Vec<Guide> {
    let len = ring.len();
    if len < EPS {
        return vec![];
    }
    // Mean width of the column between `p` and `q` (rails matched in proportion).
    let cost = |p: f64, q: f64| {
        let la = ring.fwd(p, q);
        let lb = len - la;
        (0..=END_SAMPLES)
            .map(|k| {
                let f = k as f64 / END_SAMPLES as f64;
                ring.at(p + f * la).distance(ring.at(p - f * lb))
            })
            .sum::<f64>()
    };
    let step = len / END_CANDIDATES as f64;
    let mut best = (f64::MAX, 0.0, 0.0);
    for i in 0..END_CANDIDATES {
        for j in i + 1..END_CANDIDATES {
            let (p, q) = (i as f64 * step, j as f64 * step);
            let c = cost(p, q);
            if c < best.0 {
                best = (c, p, q);
            }
        }
    }
    let fine = step / 8.0;
    let (_, p0, q0) = best;
    for di in -8..=8 {
        for dj in -8..=8 {
            let (p, q) = (p0 + di as f64 * fine, q0 + dj as f64 * fine);
            if ring.fwd(p, q) > EPS && ring.fwd(q, p) > EPS {
                let c = cost(p, q);
                if c < best.0 {
                    best = (c, p, q);
                }
            }
        }
    }
    let (point_cost, p, q) = best;
    // Over a straight end (a cut) the end guide goes along it, instead of a fan from the tip.
    let straight = stretches(ring);
    let (mut a0, mut b0) = (p, p);
    let (mut a1, mut b1) = (q, q);
    if let Some((s0, s1)) = cut(ring, &straight, p, q, true) {
        (a0, b0) = (s1, s0);
    }
    if let Some((s0, s1)) = cut(ring, &straight, q, p, false) {
        (a1, b1) = (s0, s1);
    }
    // Going forward: a0, a1, b1, b0, all the way around once; if not, the cuts overlap.
    let around = |a0: f64, a1: f64, b1: f64, b0: f64| {
        let total = ring.fwd(a0, a1) + ring.fwd(a1, b1) + ring.fwd(b1, b0) + ring.fwd(b0, a0);
        (total - len).abs() <= EPS * len.max(1.0) * 10.0
    };
    if !around(a0, a1, b1, b0) {
        (a0, b0, a1, b1) = (p, p, q, q);
    }
    // Two flat ends (the short sides of a stubby rectangle) beat two tips unless the tips are
    // clearly narrower: a tip always looks narrow, since the column starts with no width.
    let flat = |&(s, l): &(f64, f64)| {
        let d = unitario(ring.at(s + l) - ring.at(s));
        let before = unitario(ring.at(s) - ring.at(s - FLAT_PROBE));
        let after = unitario(ring.at(s + l + FLAT_PROBE) - ring.at(s + l));
        d.dot(before).abs() < CUT_COS && d.dot(after).abs() < CUT_COS
    };
    let ends: Vec<(f64, f64)> = straight.iter().filter(|s| flat(s)).copied().collect();
    let width = |a0: f64, a1: f64, b1: f64, b0: f64| {
        let (la, lb) = (ring.fwd(a0, a1), ring.fwd(b1, b0));
        (0..=END_SAMPLES)
            .map(|k| {
                let f = k as f64 / END_SAMPLES as f64;
                ring.at(a0 + f * la).distance(ring.at(b0 - f * lb))
            })
            .sum::<f64>()
    };
    let best_flat = ends
        .iter()
        .flat_map(|&(s, l)| ends.iter().map(move |&(t, m)| (s + l, t, t + m, s)))
        .filter(|&(a0, a1, b1, b0)| a1 != b0 && around(a0, a1, b1, b0))
        .map(|(a0, a1, b1, b0)| (width(a0, a1, b1, b0), a0, a1, b1, b0))
        .min_by(|x, y| x.0.total_cmp(&y.0));
    if let Some((c, f0, f1, g1, g0)) = best_flat
        && c <= FLAT_BIAS * point_cost
    {
        (a0, a1, b1, b0) = (f0, f1, g1, g0);
    }
    let (la, lb) = (ring.fwd(a0, a1), ring.fwd(b1, b0));
    let a = Arco::new(&ring.walk(a0, la));
    let b = Arco::new(&ring.walk_back(b0, lb));
    simplify(&match_rails(&a, &b, len), tolerance(len))
        .into_iter()
        .map(|(sa, sb)| [a.en(sa).0, b.en(sb).0])
        .collect()
}

/// The straight cut at tip `tip` (the column runs towards `other`), as the positions where it
/// starts and ends going forward: the straight stretch of the edge near the tip that goes most
/// across the column, if one does. `None` for pointed and round tips (the stitches fan out).
fn cut(
    ring: &Ring,
    straight: &[(f64, f64)],
    tip: f64,
    other: f64,
    forward_is_a: bool,
) -> Option<(f64, f64)> {
    let len = ring.len();
    let (la, lb) = if forward_is_a {
        (ring.fwd(tip, other), ring.fwd(other, tip))
    } else {
        (ring.fwd(other, tip), ring.fwd(tip, other))
    };
    // A rung a little into the column: its width, and a cut goes across like it.
    let (fa, fb) = if forward_is_a {
        (1.0, -1.0)
    } else {
        (-1.0, 1.0)
    };
    let rung = |t: f64| (ring.at(tip + fa * t * la), ring.at(tip + fb * t * lb));
    let (x, y) = rung(0.1);
    let width = x.distance(y).max(EPS);
    let (x, y) = rung((1.5 * width / la.max(lb)).min(0.3));
    let across = unitario(x - y);
    let near = |s: f64| ring.fwd(tip, s).min(ring.fwd(s, tip)) <= width;
    straight
        .iter()
        .filter(|&&(s0, l)| {
            // Near the tip, or with the tip in the middle.
            l <= MAX_CUT * width && (near(s0) || near(s0 + l) || ring.fwd(s0, tip) <= l)
        })
        .map(|&(s0, l)| {
            let along = unitario(ring.at(s0 + l) - ring.at(s0));
            (along.dot(across).abs(), s0, l)
        })
        .filter(|&(c, _, _)| c > CUT_COS)
        .max_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, s0, l)| (s0.rem_euclid(len), (s0 + l).rem_euclid(len)))
}

/// The straight stretches of a ring (at least [`MIN_STRETCH`] long), as (start, length).
fn stretches(ring: &Ring) -> Vec<(f64, f64)> {
    let n = ring.pts.len();
    // Start at the sharpest vertex, so no stretch goes around the start.
    let turn = |i: usize| {
        let (u, v) = (
            ring.pts[i] - ring.pts[(i + n - 1) % n],
            ring.pts[(i + 1) % n] - ring.pts[i],
        );
        u.cross(v).atan2(u.dot(v)).abs()
    };
    let start = (0..n)
        .max_by(|&i, &j| turn(i).total_cmp(&turn(j)))
        .unwrap_or(0);
    let pos = |k: usize| ring.cum[(start + k) % n] + if start + k >= n { ring.len() } else { 0.0 };
    let pt = |k: usize| ring.pts[(start + k) % n];
    let mut out = vec![];
    let mut from = 0;
    for k in 1..=n {
        let fits = |to: usize| {
            let dir = unitario(pt(to) - pt(from));
            (from + 1..to).all(|m| (pt(m) - pt(from)).cross(dir).abs() <= STRAIGHT_TOL)
        };
        if !fits(k) {
            out.push((from, k - 1));
            from = k - 1;
        }
    }
    out.push((from, n));
    out.into_iter()
        .map(|(i, j)| (pos(i), pos(j) - pos(i)))
        .filter(|&(_, l)| l >= MIN_STRETCH)
        .collect()
}

/// Automatic guides of a part with one hole: starts at the closest points of both edges and
/// goes around.
fn auto_ring(outer: &Ring, hole: &Ring) -> Vec<Guide> {
    if outer.len() < EPS || hole.len() < EPS {
        return vec![];
    }
    let n = (outer.len() / MATCH_STEP).min(MATCH_SAMPLES).ceil() as usize;
    let (so, sh) = (0..n)
        .map(|i| {
            let s = outer.len() * i as f64 / n as f64;
            let (d, h) = hole.project(outer.at(s));
            (d, s, h)
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, s, h)| (s, h))
        .unwrap_or_default();
    let a = Arco::new(&outer.walk(so, outer.len()));
    let b = Arco::new(&hole.walk(sh, hole.len()));
    let dense = match_rails(&a, &b, outer.len() + hole.len());
    let mut kept = simplify(&dense, tolerance(outer.len()));
    // The last one is the first again, around the ring.
    kept.pop();
    kept.into_iter()
        .map(|(sa, sb)| [a.en(sa).0, b.en(sb).0])
        .collect()
}

fn tolerance(len: f64) -> f64 {
    SIMPLIFY_TOL.max(1.5 * sample_step(len))
}

fn sample_step(len: f64) -> f64 {
    (len / MATCH_SAMPLES).max(MATCH_STEP)
}

/// Matches the points of both rails (dynamic time warping: monotonic, shortest total width).
/// Returns the matched positions from `(0, 0)` to both ends.
fn match_rails(a: &Arco, b: &Arco, perimeter: f64) -> Vec<(f64, f64)> {
    let step = sample_step(perimeter);
    let (la, lb) = (a.largo(), b.largo());
    let (na, nb) = (
        (la / step).ceil().max(1.0) as usize,
        (lb / step).ceil().max(1.0) as usize,
    );
    let pa: Vec<Point> = (0..=na)
        .map(|i| a.en(la * i as f64 / na as f64).0)
        .collect();
    let pb: Vec<Point> = (0..=nb)
        .map(|j| b.en(lb * j as f64 / nb as f64).0)
        .collect();
    let w = nb + 1;
    let mut acc = vec![f64::MAX; (na + 1) * w];
    for i in 0..=na {
        for j in 0..=nb {
            let prev = if i == 0 && j == 0 {
                0.0
            } else {
                let mut m = f64::MAX;
                if i > 0 {
                    m = m.min(acc[(i - 1) * w + j]);
                }
                if j > 0 {
                    m = m.min(acc[i * w + j - 1]);
                }
                if i > 0 && j > 0 {
                    m = m.min(acc[(i - 1) * w + j - 1]);
                }
                m
            };
            acc[i * w + j] = prev + pa[i].distance(pb[j]);
        }
    }
    let (mut i, mut j) = (na, nb);
    let mut path = vec![(i, j)];
    while i > 0 || j > 0 {
        let mut options = vec![];
        if i > 0 && j > 0 {
            options.push((i - 1, j - 1));
        }
        if i > 0 {
            options.push((i - 1, j));
        }
        if j > 0 {
            options.push((i, j - 1));
        }
        (i, j) = options
            .into_iter()
            .min_by(|x, y| acc[x.0 * w + x.1].total_cmp(&acc[y.0 * w + y.1]))
            .unwrap();
        path.push((i, j));
    }
    path.reverse();
    path.into_iter()
        .map(|(i, j)| (la * i as f64 / na as f64, lb * j as f64 / nb as f64))
        .collect()
}

/// Keeps the fewest rungs whose linear interpolation stays within `tol` of all of them, plus
/// evenly spaced ones so they are no more than [`MAX_GAP`] apart.
fn simplify(rungs: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    let n = rungs.len();
    if n < 3 {
        return rungs.to_vec();
    }
    let off = |m: (f64, f64), p: (f64, f64), q: (f64, f64)| {
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 {
            (((m.0 - p.0) * dx + (m.1 - p.1) * dy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ((m.0 - p.0 - t * dx).powi(2) + (m.1 - p.1 - t * dy).powi(2)).sqrt()
    };
    let mut kept = vec![rungs[0]];
    let mut i = 0;
    while i < n - 1 {
        let mut j = i + 1;
        while j + 1 < n && (i + 1..=j).all(|m| off(rungs[m], rungs[i], rungs[j + 1]) <= tol) {
            j += 1;
        }
        // Long straight stretches get evenly spaced guides too.
        let (p, q) = (rungs[i], rungs[j]);
        let k = ((q.0 - p.0).max(q.1 - p.1) / MAX_GAP).ceil().max(1.0) as usize;
        for m in 1..k {
            let u = m as f64 / k as f64;
            kept.push((p.0 + (q.0 - p.0) * u, p.1 + (q.1 - p.1) * u));
        }
        kept.push(q);
        i = j;
    }
    kept
}

/// The hole walked in the same direction as the outer ring.
fn oriented_like(hole: &Ring, outer: &Ring) -> Ring {
    if area(&hole.pts).signum() == area(&outer.pts).signum() {
        Ring::new(&hole.pts)
    } else {
        let mut pts = hole.pts.clone();
        pts.reverse();
        Ring::new(&pts)
    }
}

pub(super) fn area(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].to_vec2().cross(pts[(i + 1) % n].to_vec2()))
        .sum::<f64>()
        / 2.0
}

pub(super) fn inside_ring(pts: &[Point], p: Point) -> bool {
    let n = pts.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    inside
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

    /// Angle (degrees, 0..180) of a segment.
    fn angle(a: Point, b: Point) -> f64 {
        (b - a).atan2().to_degrees().rem_euclid(180.0)
    }

    fn column_pairs(d: &str, guides: &[Guide]) -> Vec<(Point, Point)> {
        let guides = (!guides.is_empty()).then_some(guides);
        columns(&rings(d), guides, &[], 0.4).remove(0).unwrap()
    }

    #[test]
    fn rectangle_is_sewn_across_with_flat_ends() {
        let d = "M0,0 L20,0 L20,3 L0,3 Z";
        let guides = auto_guides(&rings(d), &[]);
        assert_eq!(guides.len(), 3, "{guides:?}");
        for g in [guides[0], guides[2]] {
            // Each end guide lies on a short side.
            assert!(
                (g[0].x - g[1].x).abs() < 0.35 && (g[0].y - g[1].y).abs() > 2.9,
                "{g:?}"
            );
        }
        let pairs = column_pairs(d, &[]);
        for &(a, b) in &pairs {
            assert!((a.x - b.x).abs() < 0.35, "across: {a:?} {b:?}");
            assert!((a.y - b.y).abs() > 2.9);
        }
        assert!((pairs.len() as f64 - 101.0).abs() <= 1.0, "{}", pairs.len());
    }

    #[test]
    fn ring_goes_all_the_way_around() {
        let d =
            "M-10,0 A10,10 0 1 1 10,0 A10,10 0 1 1 -10,0 Z M-7,0 A7,7 0 1 0 7,0 A7,7 0 1 0 -7,0 Z";
        let pairs = column_pairs(d, &[]);
        let (first, last) = (pairs[0], *pairs.last().unwrap());
        assert!(first.0.distance(last.0) < 1e-6 && first.1.distance(last.1) < 1e-6);
        for &(a, b) in &pairs {
            // Every stitch goes straight across the ring, about 3 mm.
            assert!((a.distance(b) - 3.0).abs() < 0.3, "{a:?} {b:?}");
            assert!((a.to_vec2().hypot() - 10.0).abs() < 0.05);
        }
    }

    #[test]
    fn auto_guides_turn_with_an_arch() {
        // Upper half of a ring: the stitches are radial, horizontal at the ends and vertical
        // at the top.
        let d = "M-13,0 A13,13 0 0 1 13,0 L7,0 A7,7 0 0 0 -7,0 Z";
        let guides = auto_guides(&rings(d), &[]);
        assert!(guides.len() >= 3, "{guides:?}");
        let (first, last) = (guides[0], guides[guides.len() - 1]);
        assert!(angle(first[0], first[1]).min(180.0 - angle(first[0], first[1])) < 2.0);
        assert!(angle(last[0], last[1]).min(180.0 - angle(last[0], last[1])) < 2.0);
        let pairs = column_pairs(d, &[]);
        for &(a, b) in &pairs {
            // Radial: both points in line with the center.
            let sin = a.to_vec2().cross(b.to_vec2()) / (a.to_vec2().hypot() * b.to_vec2().hypot());
            assert!(sin.abs() < 0.09, "{a:?} {b:?}");
        }
        let top = pairs
            .iter()
            .min_by(|x, y| (x.0.x.abs() + x.1.x.abs()).total_cmp(&(y.0.x.abs() + y.1.x.abs())))
            .unwrap();
        assert!((angle(top.0, top.1) - 90.0).abs() < 5.0, "{top:?}");
    }

    /// Whether segments `p` and `q` cross (touching at an end doesn't count).
    fn cross(p: (Point, Point), q: (Point, Point)) -> bool {
        let side = |a: Point, b: Point, c: Point| (b - a).cross(c - a);
        let (d1, d2) = (side(q.0, q.1, p.0), side(q.0, q.1, p.1));
        let (d3, d4) = (side(p.0, p.1, q.0), side(p.0, p.1, q.1));
        d1 * d2 < -1e-9 && d3 * d4 < -1e-9
    }

    #[test]
    fn slanted_cuts_are_the_ends() {
        // A polygonal «C» with both ends cut on a slant.
        let d = "M25,-25 L8,-28.8 L-3,-25.5 L-14,-19.5 L-21.5,-12 L-26,-2 L-26.5,5 L-24,18 \
                 L-14,27 L2,28 L16,24.5 L18.5,12.3 L9,16.5 L2,17.5 L-7,16 L-11,9 L-11.5,-2 \
                 L-7,-11 L0,-16.5 L8,-18.7 L16,-17.2 L23,-13.7 Z";
        let guides = auto_guides(&rings(d), &[]);
        let is_cut = |g: &Guide, x: Point, y: Point| {
            let close = |p: Point, q: Point| p.distance(q) < 0.1;
            (close(g[0], x) && close(g[1], y)) || (close(g[0], y) && close(g[1], x))
        };
        let (top, bottom) = (
            (Point::new(25.0, -25.0), Point::new(23.0, -13.7)),
            (Point::new(16.0, 24.5), Point::new(18.5, 12.3)),
        );
        let (first, last) = (guides[0], guides[guides.len() - 1]);
        assert!(
            (is_cut(&first, top.0, top.1) && is_cut(&last, bottom.0, bottom.1))
                || (is_cut(&first, bottom.0, bottom.1) && is_cut(&last, top.0, top.1)),
            "{guides:?}"
        );
        let pairs = column_pairs(d, &[]);
        for w in pairs.windows(2) {
            assert!(!cross(w[0], w[1]), "{:?}", w);
        }
    }

    #[test]
    fn stubby_rectangles_have_flat_ends() {
        // Shorter than twice its width: still sewn between the short sides, not from a corner.
        let guides = auto_guides(&rings("M3,8.5 L10,8.5 L10,11.5 L3,11.5 Z"), &[]);
        for g in &guides {
            assert!(g[0].distance(g[1]) > 2.9, "no guide is a point: {guides:?}");
            assert!((g[0].x - g[1].x).abs() < 0.2, "across: {guides:?}");
        }
    }

    #[test]
    fn hand_guides_change_the_angle() {
        let d = "M0,0 L20,0 L20,4 L0,4 Z";
        // Ends straight; a slanted guide in the middle.
        let guides = [
            [Point::new(0.0, 0.0), Point::new(0.0, 4.0)],
            [Point::new(8.0, 0.0), Point::new(12.0, 4.0)],
            [Point::new(20.0, 0.0), Point::new(20.0, 4.0)],
        ];
        let pairs = column_pairs(d, &guides);
        let (p, q) = (guides[1][0], guides[1][1]);
        assert!(
            pairs.iter().any(|&(a, b)| {
                let near = |x: Point, y: Point| x.distance(y) < 1e-6;
                (near(a, p) && near(b, q)) || (near(a, q) && near(b, p))
            }),
            "the slanted guide is one of the stitches"
        );
        // The guides can be given in any order and either way round.
        let shuffled = [
            [guides[1][1], guides[1][0]],
            guides[2],
            [guides[0][1], guides[0][0]],
        ];
        let again = column_pairs(d, &shuffled);
        assert_eq!(again.len(), pairs.len());
    }

    #[test]
    fn parts_with_two_holes_are_skipped() {
        let d = "M0,0 L20,0 L20,10 L0,10 Z M2,2 L8,2 L8,8 L2,8 Z M12,2 L18,2 L18,8 L12,8 Z \
                 M30,0 L50,0 L50,3 L30,3 Z";
        let r = area_satin(
            &rings(d),
            None,
            &[],
            0.4,
            BaseSatin::Central,
            0.0,
            Point::ZERO,
        );
        assert_eq!(r.skipped, 1);
        assert_eq!(r.runs.len(), 1);
    }

    #[test]
    fn long_stitches_are_split() {
        let d = "M0,0 L20,0 L20,16 L0,16 Z";
        let r = area_satin(
            &rings(d),
            None,
            &[],
            0.4,
            BaseSatin::Ninguna,
            0.0,
            Point::ZERO,
        );
        for w in r.runs[0].windows(2) {
            assert!(w[0].distance(w[1]) <= MAX_STITCH + 1e-9);
        }
    }
}
