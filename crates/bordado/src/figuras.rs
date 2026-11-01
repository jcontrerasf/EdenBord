//! Parametric shapes ([`crate::Forma::Shape`]): rectangle, ellipse, regular polygon, star and
//! a few typical figures. Each one fills its `width × height` box exactly, centered on the
//! origin; position, rotation and mirror go in the object's affine.

use std::f64::consts::{PI, TAU};

use kurbo::{Affine, BezPath, Ellipse, Point, Rect, RoundedRect, Shape as _, Vec2};
use serde::{Deserialize, Serialize};

use crate::geom::TOLERANCIA;

/// Fewest and most sides of a polygon (or points of a star).
pub const MIN_SIDES: u32 = 3;
pub const MAX_SIDES: u32 = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo")]
pub enum Shape {
    /// Rectangle; `radius` rounds the corners (0 = square corners).
    #[serde(rename = "rectangulo")]
    Rectangle {
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
        #[serde(rename = "radio", default)]
        radius: f64,
    },
    #[serde(rename = "elipse")]
    Ellipse {
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
    },
    /// Regular polygon (with its natural proportion), first vertex up.
    #[serde(rename = "poligono")]
    Polygon {
        #[serde(rename = "lados")]
        sides: u32,
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
    },
    /// Star; `inner` = radius of the inner vertices over the outer ones.
    #[serde(rename = "estrella")]
    Star {
        #[serde(rename = "puntas")]
        points: u32,
        #[serde(rename = "interior")]
        inner: f64,
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
    },
    #[serde(rename = "corazon")]
    Heart {
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
    },
    #[serde(rename = "gota")]
    Drop {
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
    },
    #[serde(rename = "flecha")]
    Arrow {
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
    },
    #[serde(rename = "hoja")]
    Leaf {
        #[serde(rename = "ancho")]
        width: f64,
        #[serde(rename = "alto")]
        height: f64,
    },
}

/// Size of a new shape when it is placed with a click, in mm (the larger side).
pub const DEFAULT_SIZE: f64 = 30.0;

const HEART: &str = "M50,30 C50,27 45,15 25,15 C0,15 0,42.5 0,42.5 C0,60 20,77 50,95 \
                     C80,77 100,60 100,42.5 C100,42.5 100,15 75,15 C60,15 50,27 50,30 Z";
const DROP: &str = "M50,0 C50,0 15,45 15,65 C15,85 32,100 50,100 C68,100 85,85 85,65 \
                    C85,45 50,0 50,0 Z";
const ARROW: &str = "M0,35 L60,35 L60,10 L100,50 L60,90 L60,65 L0,65 Z";
const LEAF: &str = "M50,0 C90,30 80,80 50,100 C20,80 10,30 50,0 Z";

impl Shape {
    /// One of each kind, at its natural proportion and [`DEFAULT_SIZE`], in the order shown.
    pub fn all() -> [Shape; 8] {
        let s = DEFAULT_SIZE;
        [
            Shape::Rectangle {
                width: s,
                height: s,
                radius: 0.0,
            },
            Shape::Ellipse {
                width: s,
                height: s,
            },
            Shape::Polygon {
                sides: 6,
                width: s,
                height: s,
            },
            Shape::Star {
                points: 5,
                inner: 0.45,
                width: s,
                height: s,
            },
            Shape::Heart {
                width: s,
                height: s,
            },
            Shape::Drop {
                width: s,
                height: s,
            },
            Shape::Arrow {
                width: s,
                height: s,
            },
            Shape::Leaf {
                width: s,
                height: s,
            },
        ]
        .map(|f| {
            let r = f.ratio();
            let (w, h) = if r >= 1.0 { (s, s / r) } else { (s * r, s) };
            f.with_size(w, h)
        })
    }

    /// Name for the interface.
    pub fn name(&self) -> &'static str {
        match self {
            Shape::Rectangle { .. } => "Rectángulo",
            Shape::Ellipse { .. } => "Elipse",
            Shape::Polygon { .. } => "Polígono",
            Shape::Star { .. } => "Estrella",
            Shape::Heart { .. } => "Corazón",
            Shape::Drop { .. } => "Gota",
            Shape::Arrow { .. } => "Flecha",
            Shape::Leaf { .. } => "Hoja",
        }
    }

    /// Whether both are the same kind of shape (other parameters may differ).
    pub fn same_kind(&self, other: &Shape) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    pub fn size(&self) -> (f64, f64) {
        match *self {
            Shape::Rectangle { width, height, .. }
            | Shape::Ellipse { width, height }
            | Shape::Polygon { width, height, .. }
            | Shape::Star { width, height, .. }
            | Shape::Heart { width, height }
            | Shape::Drop { width, height }
            | Shape::Arrow { width, height }
            | Shape::Leaf { width, height } => (width, height),
        }
    }

    pub fn size_mut(&mut self) -> (&mut f64, &mut f64) {
        match self {
            Shape::Rectangle { width, height, .. }
            | Shape::Ellipse { width, height }
            | Shape::Polygon { width, height, .. }
            | Shape::Star { width, height, .. }
            | Shape::Heart { width, height }
            | Shape::Drop { width, height }
            | Shape::Arrow { width, height }
            | Shape::Leaf { width, height } => (width, height),
        }
    }

    pub fn with_size(mut self, w: f64, h: f64) -> Shape {
        let (width, height) = self.size_mut();
        (*width, *height) = (w, h);
        self
    }

    /// Natural width / height: a regular polygon or star, a circle, an upright heart.
    pub fn ratio(&self) -> f64 {
        match self.unit() {
            Some(path) => {
                let b = path.bounding_box();
                b.width() / b.height()
            }
            None => 1.0,
        }
    }

    /// The outline, centered on the origin, filling `width × height` exactly.
    pub fn path(&self) -> BezPath {
        let (w, h) = self.size();
        let (w, h) = (w.max(0.01), h.max(0.01));
        let rect = Rect::new(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0);
        match *self {
            Shape::Rectangle { radius, .. } => {
                let r = radius.clamp(0.0, w.min(h) / 2.0);
                if r <= 0.0 {
                    rect.to_path(TOLERANCIA)
                } else {
                    RoundedRect::from_rect(rect, r).to_path(TOLERANCIA)
                }
            }
            Shape::Ellipse { .. } => {
                Ellipse::new(Point::ZERO, Vec2::new(w / 2.0, h / 2.0), 0.0).to_path(TOLERANCIA)
            }
            _ => {
                let mut path = self.unit().expect("the other shapes have a unit path");
                let b = path.bounding_box();
                path.apply_affine(
                    Affine::translate(rect.center().to_vec2())
                        * Affine::scale_non_uniform(w / b.width(), h / b.height())
                        * Affine::translate(-b.center().to_vec2()),
                );
                path
            }
        }
    }

    /// The shape at any size and position (fitted to its box by [`path`](Self::path)); `None`
    /// for the rectangle and the ellipse, which are drawn directly.
    fn unit(&self) -> Option<BezPath> {
        let svg = |d: &str| BezPath::from_svg(d).expect("valid built-in path");
        Some(match *self {
            Shape::Rectangle { .. } | Shape::Ellipse { .. } => return None,
            Shape::Polygon { sides, .. } => {
                let n = sides.clamp(MIN_SIDES, MAX_SIDES);
                polygon((0..n).map(|k| (1.0, f64::from(k) / f64::from(n))))
            }
            Shape::Star { points, inner, .. } => {
                let n = points.clamp(MIN_SIDES, MAX_SIDES);
                let inner = inner.clamp(0.05, 0.99);
                polygon((0..2 * n).map(|k| {
                    let r = if k % 2 == 0 { 1.0 } else { inner };
                    (r, f64::from(k) / f64::from(2 * n))
                }))
            }
            Shape::Heart { .. } => svg(HEART),
            Shape::Drop { .. } => svg(DROP),
            Shape::Arrow { .. } => svg(ARROW),
            Shape::Leaf { .. } => svg(LEAF),
        })
    }

    /// Takes the scale out of `afin` (if it has no shear) into the width and height, so that
    /// scaling a rounded rectangle doesn't stretch its corners. `afin` keeps position,
    /// rotation and mirror.
    pub fn normalize(&mut self, afin: &mut [f64; 6]) {
        let [a, b, c, d, e, f] = *afin;
        let (sx, sy) = (a.hypot(b), c.hypot(d));
        if sx < 1e-9 || sy < 1e-9 || (a * c + b * d).abs() > 1e-9 * sx * sy {
            return;
        }
        let (width, height) = self.size_mut();
        *width *= sx;
        *height *= sy;
        *afin = [a / sx, b / sx, c / sy, d / sy, e, f];
    }
}

/// Closed polygon from (radius, fraction of a turn) pairs, starting straight up.
fn polygon(vertices: impl Iterator<Item = (f64, f64)>) -> BezPath {
    let mut path = BezPath::new();
    for (i, (r, t)) in vertices.enumerate() {
        let a = t * TAU - PI / 2.0;
        let p = Point::new(r * a.cos(), r * a.sin());
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    path.close_path();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::PathEl;

    fn vertices(path: &BezPath) -> usize {
        path.elements()
            .iter()
            .filter(|e| matches!(e, PathEl::MoveTo(_) | PathEl::LineTo(_)))
            .count()
    }

    #[test]
    fn every_shape_fills_its_box() {
        for shape in Shape::all() {
            for (w, h) in [(30.0, 20.0), (10.0, 40.0)] {
                let b = shape.clone().with_size(w, h).path().bounding_box();
                let msg = shape.name();
                assert!((b.width() - w).abs() < 1e-6, "{msg}: {b:?}");
                assert!((b.height() - h).abs() < 1e-6, "{msg}: {b:?}");
                assert!(b.center().to_vec2().hypot() < 1e-6, "{msg}: {b:?}");
            }
        }
    }

    #[test]
    fn polygons_and_stars_have_their_vertices() {
        for n in MIN_SIDES..=MAX_SIDES {
            let p = Shape::Polygon {
                sides: n,
                width: 10.0,
                height: 10.0,
            };
            assert_eq!(vertices(&p.path()), n as usize);
            let s = Shape::Star {
                points: n,
                inner: 0.5,
                width: 10.0,
                height: 10.0,
            };
            assert_eq!(vertices(&s.path()), 2 * n as usize);
        }
    }

    #[test]
    fn natural_ratio_gives_regular_polygons() {
        for n in MIN_SIDES..=MAX_SIDES {
            let p = Shape::Polygon {
                sides: n,
                width: 1.0,
                height: 1.0,
            };
            let r = p.ratio();
            let path = p.with_size(20.0 * r, 20.0).path();
            let pts: Vec<Point> = path
                .elements()
                .iter()
                .filter_map(|e| match e {
                    PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(*p),
                    _ => None,
                })
                .collect();
            let sides: Vec<f64> = (0..pts.len())
                .map(|i| pts[i].distance(pts[(i + 1) % pts.len()]))
                .collect();
            for s in &sides {
                assert!((s - sides[0]).abs() < 1e-9, "{n}: {sides:?}");
            }
        }
        // Every shape from `all` keeps its natural proportion.
        for f in Shape::all() {
            let (w, h) = f.size();
            assert!((w / h - f.ratio()).abs() < 1e-9, "{}", f.name());
            assert!((w.max(h) - DEFAULT_SIZE).abs() < 1e-9);
        }
    }

    #[test]
    fn normalize_moves_scale_into_the_size() {
        let mut f = Shape::Rectangle {
            width: 10.0,
            height: 4.0,
            radius: 0.0,
        };
        let t = Affine::translate((5.0, 6.0))
            * Affine::rotate(0.3)
            * Affine::scale_non_uniform(2.0, -3.0);
        let before = {
            let mut p = f.path();
            p.apply_affine(t);
            p.bounding_box()
        };
        let mut afin = t.as_coeffs();
        f.normalize(&mut afin);
        assert_eq!(f.size(), (20.0, 12.0));
        let after = {
            let mut p = f.path();
            p.apply_affine(Affine::new(afin));
            p.bounding_box()
        };
        assert!((before.x0 - after.x0).abs() < 1e-6 && (before.y1 - after.y1).abs() < 1e-6);
        // With shear the affine is left alone.
        let mut sheared = (Affine::scale_non_uniform(2.0, 1.0) * Affine::rotate(0.5)).as_coeffs();
        let copy = sheared;
        f.normalize(&mut sheared);
        assert_eq!(sheared, copy);
    }
}
