use kurbo::Point;

use crate::geom::{Arco, Polilinea, normal, recorrido, remuestrear, unitario};
use crate::proyecto::BaseSatin;

use super::Tramo;

/// Largo de puntada de las bases.
const LARGO_BASE: f64 = 2.0;
/// Cuánto se mete la base de contorno desde cada borde.
const MARGEN_BASE: f64 = 0.4;
/// Distancia entre puntas del zigzag de base, y qué parte del ancho cubre.
const PASO_ZIGZAG: f64 = 2.0;
const ANCHO_ZIGZAG: f64 = 0.7;
/// Ancho de las puntas del satín ahusado, en fracción del ancho.
const PUNTA: f64 = 0.15;
/// En una curva, el lado de adentro no pasa de esta fracción del radio (si no, se cruza).
const RADIO: f64 = 0.9;

/// Columna en zigzag centrada en cada polilínea. La base se cose primero y el satín vuelve
/// sobre ella; con base central o zigzag termina donde empezó.
pub fn satin(
    lineas: &[Polilinea],
    ancho: f64,
    densidad: f64,
    base: BaseSatin,
    compensacion: f64,
    ahusado: bool,
) -> Vec<Tramo> {
    lineas
        .iter()
        .map(|pl| {
            let bordes = bordes(pl, ancho, densidad, compensacion, ahusado);
            let zigzag: Vec<Point> = bordes
                .iter()
                .enumerate()
                .map(|(i, &(izq, der))| if i % 2 == 0 { izq } else { der })
                .collect();
            let centro: Vec<Point> = bordes.iter().map(|(a, b)| a.midpoint(*b)).collect();
            let adentro = |p: Point, c: Point| {
                let d = p - c;
                let h = d.hypot();
                c + unitario(d) * (h - MARGEN_BASE).max(0.0)
            };
            match base {
                BaseSatin::Ninguna => zigzag,
                BaseSatin::Central => con_vuelta(remuestrear(&centro, LARGO_BASE), zigzag),
                BaseSatin::Zigzag => {
                    let cada = ((PASO_ZIGZAG / densidad).round() as usize).max(2);
                    let ralo: Vec<Point> = bordes
                        .iter()
                        .zip(&centro)
                        .enumerate()
                        .filter(|(i, _)| i % cada == 0 || *i == bordes.len() - 1)
                        .enumerate()
                        .map(|(k, (_, (&(izq, der), &c)))| {
                            let p = if k % 2 == 0 { izq } else { der };
                            c.lerp(p, ANCHO_ZIGZAG)
                        })
                        .collect();
                    con_vuelta(ralo, zigzag)
                }
                BaseSatin::Contorno => {
                    // Por un borde hacia adelante, por el otro de vuelta, y el satín de nuevo
                    // hacia adelante.
                    let ida: Vec<Point> = bordes
                        .iter()
                        .zip(&centro)
                        .map(|(&(izq, _), &c)| adentro(izq, c))
                        .collect();
                    let vuelta: Vec<Point> = bordes
                        .iter()
                        .zip(&centro)
                        .rev()
                        .map(|(&(_, der), &c)| adentro(der, c))
                        .collect();
                    let mut out = remuestrear(&ida, LARGO_BASE);
                    out.extend(remuestrear(&vuelta, LARGO_BASE));
                    out.extend(zigzag);
                    out
                }
            }
        })
        .collect()
}

/// La base hacia adelante y el satín de vuelta.
fn con_vuelta(mut base: Vec<Point>, mut zigzag: Vec<Point>) -> Vec<Point> {
    zigzag.reverse();
    base.extend(zigzag);
    base
}

/// Los dos bordes (izquierdo, derecho) en cada punto del centro, separados `densidad / 2`.
fn bordes(
    pl: &Polilinea,
    ancho: f64,
    densidad: f64,
    compensacion: f64,
    ahusado: bool,
) -> Vec<(Point, Point)> {
    let ahusado = ahusado && !pl.cerrada;
    let centro = remuestrear(&recorrido(pl), densidad / 2.0);
    let arco = Arco::new(&centro);
    let total = arco.largo();
    let n = centro.len();
    let mut s = 0.0;
    (0..n)
        .map(|i| {
            if i > 0 {
                s += centro[i - 1].distance(centro[i]);
            }
            let (a, b) = if pl.cerrada && (i == 0 || i == n - 1) {
                // El primero y el último son el mismo punto: tangente a través del cierre.
                (centro[n - 2], centro[1])
            } else {
                (centro[i.saturating_sub(1)], centro[(i + 1).min(n - 1)])
            };
            let t = unitario(b - a);
            let mut h = ancho / 2.0;
            if ahusado {
                let largo_punta = (total / 2.0).min(2.5 * ancho).max(1e-9);
                let x = (s.min(total - s) / largo_punta).clamp(0.0, 1.0);
                h *= PUNTA + (1.0 - PUNTA) * x * x * (3.0 - 2.0 * x);
            }
            h += compensacion;
            let (mut izq, mut der) = (h, h);
            // Curvatura: con el círculo que pasa por el centro y dos puntos a ±h.
            if let Some((radio, hacia_izq)) = curva(&arco, s, h.max(densidad)) {
                let lado = if hacia_izq { &mut izq } else { &mut der };
                *lado = lado.min(RADIO * radio);
            }
            let nrm = normal(t);
            (centro[i] + nrm * izq, centro[i] - nrm * der)
        })
        .collect()
}

/// Radio de la curva en `s` y si gira hacia la izquierda. `None` si es casi recta. Mira de
/// `s - d` a `s + d`, corrido hacia adentro cerca de las puntas.
fn curva(arco: &Arco, s: f64, d: f64) -> Option<(f64, bool)> {
    let total = arco.largo();
    let desde = (s - d).clamp(0.0, (total - 2.0 * d).max(0.0));
    let hasta = (desde + 2.0 * d).min(total);
    let (a, b, c) = (
        arco.en(desde).0,
        arco.en((desde + hasta) / 2.0).0,
        arco.en(hasta).0,
    );
    let cruz = (b - a).cross(c - b);
    let lados = a.distance(b) * b.distance(c) * a.distance(c);
    if cruz.abs() < 1e-9 || lados < 1e-12 {
        return None;
    }
    // Con y hacia abajo, girar a la izquierda es cruz negativa.
    Some((lados / (2.0 * cruz.abs()), cruz < 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recta() -> Polilinea {
        Polilinea {
            puntos: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            cerrada: false,
        }
    }

    #[test]
    fn zigzag_alternates_sides() {
        let pl = recta();
        let t = &satin(
            std::slice::from_ref(&pl),
            3.0,
            0.5,
            BaseSatin::Ninguna,
            0.0,
            false,
        )[0];
        assert_eq!(t.len(), 41);
        for (i, p) in t.iter().enumerate() {
            let lado = if i % 2 == 0 { -1.5 } else { 1.5 };
            assert!((p.y - lado).abs() < 1e-9);
        }
        let con_base = &satin(&[pl], 3.0, 0.5, BaseSatin::Central, 0.0, false)[0];
        assert_eq!(con_base.len(), 6 + 41);
        assert_eq!(con_base[5], Point::new(10.0, 0.0));
    }

    #[test]
    fn compensation_and_taper_change_the_width() {
        let pl = recta();
        let ancho = |t: &Tramo, i: usize| (t[i].y - t[i + 1].y).abs();
        let comp = &satin(
            std::slice::from_ref(&pl),
            3.0,
            0.5,
            BaseSatin::Ninguna,
            0.2,
            false,
        )[0];
        assert!((ancho(comp, 20) - 3.4).abs() < 1e-9);
        let fino = &satin(&[pl], 3.0, 0.5, BaseSatin::Ninguna, 0.0, true)[0];
        assert!(
            ancho(fino, 0) < 0.5 && ancho(fino, 39) < 0.5,
            "puntas finas"
        );
        assert!(
            (ancho(fino, 20) - 3.0).abs() < 0.05,
            "centro con el ancho entero"
        );
    }

    #[test]
    fn bases_stay_inside_and_contour_walks_both_edges() {
        for base in BaseSatin::TODAS {
            let t = &satin(&[recta()], 4.0, 0.4, base, 0.0, false)[0];
            assert!(t.iter().all(|p| p.y.abs() <= 2.0 + 1e-9), "{base:?}");
        }
        let t = &satin(&[recta()], 4.0, 0.4, BaseSatin::Contorno, 0.0, false)[0];
        assert!(t.iter().any(|p| (p.y + 1.6).abs() < 1e-9));
        assert!(t.iter().any(|p| (p.y - 1.6).abs() < 1e-9));
    }

    #[test]
    fn tight_curve_does_not_cross_inside() {
        // Medio círculo de radio 2 con un satín de 6 mm: el lado de adentro se recorta.
        let d = kurbo::BezPath::from_svg("M2,0 A2,2 0 0 1 -2,0").unwrap();
        let pl = crate::geom::polilineas(&d).remove(0);
        let t = &satin(&[pl], 6.0, 0.4, BaseSatin::Ninguna, 0.0, false)[0];
        // El lado de adentro no llega al centro del círculo (ni lo pasa).
        assert!(t.iter().all(|p| p.distance(Point::ZERO) > 0.15), "{t:?}");
        assert!(t.iter().all(|p| p.distance(Point::ZERO) <= 5.0 + 1e-6));
    }
}
