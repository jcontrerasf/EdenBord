//! Gestos del ratón sobre un proyecto: mover, escalar con manijas, girar y seleccionar con
//! un rectángulo. Mientras dura el gesto solo se acumula la transformación (`af`); se aplica
//! al soltar, con un único punto de deshacer.

use kurbo::{Affine, Point, Rect, Vec2};

/// Tamaño mínimo (mm) al que se puede achicar con una manija.
const MIN_MM: f64 = 0.5;
/// Paso del giro con Shift, en grados.
const GIRO_PASO: f64 = 15.0;

/// A qué se aplica el gesto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blanco {
    /// Los objetos seleccionados.
    Objetos,
    /// La imagen de fondo.
    Fondo,
}

/// Qué hace el arrastre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Mover,
    /// Manija de escala: `(sx, sy)` en {-1, 0, 1} = lado del rectángulo (0 = al medio).
    /// Esquinas: ambos distintos de 0; lados: uno es 0.
    Escalar(i8, i8),
    Girar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesto {
    Transformar {
        blanco: Blanco,
        tipo: Tipo,
        /// Rectángulo (mm) del blanco al empezar.
        r0: Rect,
        /// Dónde se apretó (mm).
        desde: Point,
        /// Transformación acumulada desde el inicio.
        af: Affine,
    },
    /// Selección con un rectángulo de `a` a `b` (mm). `sumar`: agregar a la selección.
    Caja { a: Point, b: Point, sumar: bool },
    /// Dirección de las filas del relleno del objeto `obj`, girando alrededor de `centro`
    /// (mm). El ángulo se aplica en vivo (edición agrupada), `angulo` es el actual.
    Angulo {
        obj: usize,
        centro: Point,
        angulo: f64,
        /// El ángulo al empezar: si cambió, cancelar deshace la edición.
        inicial: f64,
    },
}

/// Modificadores que cambian el gesto.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

impl Gesto {
    /// Actualiza el gesto con la posición actual del puntero (mm).
    pub fn arrastrar(&mut self, hasta: Point, mods: Mods) {
        match self {
            Gesto::Transformar {
                blanco,
                tipo,
                r0,
                desde,
                af,
            } => {
                *af = match *tipo {
                    Tipo::Mover => afin_mover(*desde, hasta, mods.ctrl),
                    Tipo::Escalar(sx, sy) => {
                        // El fondo solo tiene ancho: siempre proporcional y sin lados.
                        let libre = mods.shift && *blanco == Blanco::Objetos;
                        afin_escala((sx, sy), *r0, *desde, hasta, libre, mods.ctrl)
                    }
                    Tipo::Girar => afin_giro(r0.center(), *desde, hasta, mods.shift),
                }
            }
            Gesto::Caja { b, .. } => *b = hasta,
            Gesto::Angulo { centro, angulo, .. } => {
                *angulo = angulo_filas(*centro, hasta, mods.shift);
            }
        }
    }
}

/// Ángulo de las filas (grados, en (−90, 90]: una fila no tiene sentido) que apunta de `centro`
/// a `hacia`. Positivo = horario en pantalla, como `Puntada::Relleno::angulo`. Con `paso`, de
/// a 15°.
pub fn angulo_filas(centro: Point, hacia: Point, paso: bool) -> f64 {
    let d = hacia - centro;
    let mut a = d.y.atan2(d.x).to_degrees();
    if paso {
        a = (a / GIRO_PASO).round() * GIRO_PASO;
    }
    while a > 90.0 {
        a -= 180.0;
    }
    while a <= -90.0 {
        a += 180.0;
    }
    if a.abs() < 1e-9 { 0.0 } else { a }
}

/// Mueve de `desde` a `hasta`; con `recto`, solo en horizontal o vertical (el eje mayor).
pub fn afin_mover(desde: Point, hasta: Point, recto: bool) -> Affine {
    let mut d = hasta - desde;
    if recto {
        if d.x.abs() >= d.y.abs() {
            d.y = 0.0;
        } else {
            d.x = 0.0;
        }
    }
    Affine::translate(d)
}

/// Escala arrastrando la manija `h` de `r0` de `desde` a `hasta`. La referencia fija es el
/// lado o la esquina opuesta (con `centro`, el centro). En las esquinas es proporcional salvo
/// con `libre`. Nunca invierte ni achica por debajo de [`MIN_MM`].
pub fn afin_escala(
    h: (i8, i8),
    r0: Rect,
    desde: Point,
    hasta: Point,
    libre: bool,
    centro: bool,
) -> Affine {
    let lado = |s: i8, min: f64, max: f64| match s {
        1 => max,
        -1 => min,
        _ => (min + max) / 2.0,
    };
    let manija = Point::new(lado(h.0, r0.x0, r0.x1), lado(h.1, r0.y0, r0.y1));
    let fijo = if centro {
        r0.center()
    } else {
        Point::new(lado(-h.0, r0.x0, r0.x1), lado(-h.1, r0.y0, r0.y1))
    };
    let nueva = manija + (hasta - desde);
    let factor = |s: i8, a: f64, b: f64, f: f64, largo: f64| {
        let d = a - f;
        if s == 0 || d.abs() < 1e-9 {
            return 1.0;
        }
        let min = MIN_MM / largo.max(MIN_MM);
        ((b - f) / d).max(min)
    };
    let mut fx = factor(h.0, manija.x, nueva.x, fijo.x, r0.width());
    let mut fy = factor(h.1, manija.y, nueva.y, fijo.y, r0.height());
    if h.0 != 0 && h.1 != 0 && !libre {
        // Proporcional: manda el eje que más cambió.
        let f = if (fx - 1.0).abs() >= (fy - 1.0).abs() {
            fx
        } else {
            fy
        };
        // Un rectángulo sin ancho (o sin alto) no tiene factor en ese eje.
        fx = if r0.width() < 1e-9 { 1.0 } else { f };
        fy = if r0.height() < 1e-9 { 1.0 } else { f };
    }
    let v = fijo.to_vec2();
    Affine::translate(v) * Affine::scale_non_uniform(fx, fy) * Affine::translate(-v)
}

/// Gira alrededor de `c` lo que gira el puntero de `desde` a `hasta`; con `paso`, de a 15°.
pub fn afin_giro(c: Point, desde: Point, hasta: Point, paso: bool) -> Affine {
    let ang = |p: Point| {
        let v: Vec2 = p - c;
        v.y.atan2(v.x)
    };
    let mut a = (ang(hasta) - ang(desde)).to_degrees();
    if paso {
        a = (a / GIRO_PASO).round() * GIRO_PASO;
    }
    Affine::rotate_about(a.to_radians(), c)
}

/// Si `dentro` está completamente contenido en `r`.
pub fn contiene(r: Rect, dentro: Rect) -> bool {
    r.x0 <= dentro.x0 && r.y0 <= dentro.y0 && dentro.x1 <= r.x1 && dentro.y1 <= r.y1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Rect {
        Rect::new(0.0, 0.0, 20.0, 10.0)
    }

    fn near(a: Point, b: Point) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn corner_scales_proportionally_from_opposite_corner() {
        // Esquina inferior derecha 10 mm a la derecha: 1,5× (manda el eje que más cambió).
        let t = afin_escala(
            (1, 1),
            r(),
            Point::new(20.0, 10.0),
            Point::new(30.0, 10.0),
            false,
            false,
        );
        assert!(
            near(t * Point::ZERO, Point::ZERO),
            "la esquina opuesta queda fija"
        );
        assert!(near(t * Point::new(20.0, 10.0), Point::new(30.0, 15.0)));

        // Con Shift (libre) solo cambia el ancho.
        let t = afin_escala(
            (1, 1),
            r(),
            Point::new(20.0, 10.0),
            Point::new(30.0, 10.0),
            true,
            false,
        );
        assert!(near(t * Point::new(20.0, 10.0), Point::new(30.0, 10.0)));

        // Con Ctrl, desde el centro: la manija se mueve 10, el ancho medio pasa de 10 a 20.
        let t = afin_escala(
            (1, 1),
            r(),
            Point::new(20.0, 10.0),
            Point::new(30.0, 10.0),
            false,
            true,
        );
        assert!(near(t * Point::new(10.0, 5.0), Point::new(10.0, 5.0)));
        assert!(near(t * Point::new(20.0, 10.0), Point::new(30.0, 15.0)));
    }

    #[test]
    fn side_scales_one_axis_and_never_flips() {
        let t = afin_escala(
            (0, -1),
            r(),
            Point::new(10.0, 0.0),
            Point::new(10.0, -10.0),
            false,
            false,
        );
        assert!(near(t * Point::new(20.0, 0.0), Point::new(20.0, -10.0)));
        assert!(near(t * Point::new(20.0, 10.0), Point::new(20.0, 10.0)));

        // Arrastrar más allá del lado opuesto deja el mínimo, sin invertir.
        let t = afin_escala(
            (1, 0),
            r(),
            Point::new(20.0, 5.0),
            Point::new(-30.0, 5.0),
            false,
            false,
        );
        let ancho = (t * Point::new(20.0, 0.0)).x - (t * Point::ZERO).x;
        assert!((ancho - MIN_MM).abs() < 1e-9);
    }

    #[test]
    fn flat_shapes_keep_their_flat_axis() {
        let linea = Rect::new(0.0, 0.0, 20.0, 0.0);
        let t = afin_escala(
            (1, 1),
            linea,
            Point::new(20.0, 0.0),
            Point::new(40.0, 0.0),
            false,
            false,
        );
        assert!(near(t * Point::new(20.0, 0.0), Point::new(40.0, 0.0)));
        assert!(t.as_coeffs().iter().all(|c| c.is_finite()));
    }

    #[test]
    fn move_and_rotate() {
        let t = afin_mover(Point::ZERO, Point::new(3.0, 1.0), true);
        assert!(near(t * Point::ZERO, Point::new(3.0, 0.0)));
        let t = afin_mover(Point::ZERO, Point::new(1.0, -3.0), true);
        assert!(near(t * Point::ZERO, Point::new(0.0, -3.0)));

        let c = Point::new(10.0, 10.0);
        let t = afin_giro(c, Point::new(20.0, 10.0), Point::new(10.0, 20.0), false);
        assert!(near(t * Point::new(20.0, 10.0), Point::new(10.0, 20.0)));
        // 50° con paso de 15° queda en 45°.
        let a = 50f64.to_radians();
        let t = afin_giro(
            c,
            Point::new(20.0, 10.0),
            c + Vec2::new(a.cos(), a.sin()),
            true,
        );
        let p = t * Point::new(20.0, 10.0) - c;
        assert!((p.y.atan2(p.x).to_degrees() - 45.0).abs() < 1e-9);
    }

    #[test]
    fn gesture_tracks_pointer_and_background_stays_proportional() {
        let mut g = Gesto::Transformar {
            blanco: Blanco::Fondo,
            tipo: Tipo::Escalar(1, 1),
            r0: r(),
            desde: Point::new(20.0, 10.0),
            af: Affine::IDENTITY,
        };
        g.arrastrar(
            Point::new(40.0, 10.0),
            Mods {
                shift: true,
                ctrl: false,
            },
        );
        let Gesto::Transformar { af, .. } = g else {
            unreachable!()
        };
        assert!(near(af * Point::new(20.0, 10.0), Point::new(40.0, 20.0)));
        assert!(contiene(r(), Rect::new(1.0, 1.0, 2.0, 2.0)));
        assert!(!contiene(r(), Rect::new(1.0, 1.0, 21.0, 2.0)));
    }

    #[test]
    fn fill_angle_points_at_the_pointer() {
        let c = Point::ZERO;
        assert_eq!(angulo_filas(c, Point::new(10.0, 0.0), false), 0.0);
        assert_eq!(angulo_filas(c, Point::new(-10.0, 0.0), false), 0.0);
        // y hacia abajo: abajo a la derecha es horario (positivo).
        assert!((angulo_filas(c, Point::new(10.0, 10.0), false) - 45.0).abs() < 1e-9);
        assert!((angulo_filas(c, Point::new(-10.0, 10.0), false) + 45.0).abs() < 1e-9);
        assert_eq!(angulo_filas(c, Point::new(0.0, 10.0), false), 90.0);
        assert_eq!(angulo_filas(c, Point::new(0.0, -10.0), false), 90.0);
        assert_eq!(angulo_filas(c, Point::new(10.0, 3.0), true), 15.0);
    }
}
