//! Relleno tatami: filas paralelas cosidas en serpentina.
//!
//! Se gira la forma para que las filas queden horizontales, se cortan con líneas de barrido
//! (regla par-impar, así los agujeros quedan vacíos) y los segmentos se agrupan en secciones
//! que se pueden coser en serpentina sin atravesar huecos: un segmento sigue la sección de la
//! fila anterior solo si ambos se solapan entre sí y con ningún otro. Cada sección es un tramo.

use kurbo::{Affine, Point};

use crate::geom::subdividir;

use super::Tramo;

/// Desfase de las puntadas en filas sucesivas, en fracciones del largo (patrón de ladrillo).
const DESFASES: [f64; 4] = [0.0, 0.5, 0.25, 0.75];
/// Una puntada interior no queda más cerca que esto (fracción del largo) del borde de la fila.
const MARGEN: f64 = 0.3;
/// Segmentos más cortos que esto (mm) no se cosen.
const MINIMO: f64 = 0.2;

/// Rellena los anillos (cerrados, sin repetir el primer punto). `desde` es donde está la
/// aguja, para empezar por la sección más cercana.
pub fn relleno(
    anillos: &[Vec<Point>],
    angulo: f64,
    separacion: f64,
    largo: f64,
    compensacion: f64,
    desde: Point,
) -> Vec<Tramo> {
    let giro = Affine::rotate(-angulo.to_radians());
    let anillos: Vec<Vec<Point>> = anillos
        .iter()
        .filter(|a| a.len() > 2)
        .map(|a| a.iter().map(|&p| giro * p).collect())
        .collect();
    let (ymin, ymax) = anillos
        .iter()
        .flatten()
        .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    if anillos.is_empty() || separacion <= 0.0 || largo <= 0.0 {
        return vec![];
    }

    // Filas centradas en la altura de la forma.
    let n = ((ymax - ymin) / separacion).floor().max(1.0) as usize;
    let y0 = (ymin + ymax) / 2.0 - (n - 1) as f64 * separacion / 2.0;
    let filas: Vec<Vec<(f64, f64)>> = (0..n)
        .map(|k| segmentos(&anillos, y0 + k as f64 * separacion, compensacion))
        .collect();

    let secciones = secciones(&filas);
    let inversa = giro.inverse();
    let mut pos = giro * desde;
    let mut pendientes: Vec<&Vec<(usize, f64, f64)>> = secciones.iter().collect();
    let mut out = Vec::new();
    while !pendientes.is_empty() {
        // Sección y esquina de inicio más cercanas a la aguja.
        let (i, desde_abajo, al_reves) = (0..pendientes.len())
            .flat_map(|i| {
                [
                    (i, false, false),
                    (i, false, true),
                    (i, true, false),
                    (i, true, true),
                ]
            })
            .min_by(|&a, &b| {
                let d = |(i, abajo, rev): (usize, bool, bool)| {
                    let s = pendientes[i];
                    let &(k, x0, x1) = if abajo { s.last() } else { s.first() }.unwrap();
                    let x = if rev { x1 } else { x0 };
                    pos.distance(Point::new(x, y0 + k as f64 * separacion))
                };
                d(a).total_cmp(&d(b))
            })
            .unwrap();
        let seccion = pendientes.swap_remove(i);
        let orden: Vec<_> = if desde_abajo {
            seccion.iter().rev().collect()
        } else {
            seccion.iter().collect()
        };
        let mut tramo = Vec::new();
        for (j, &&(k, x0, x1)) in orden.iter().enumerate() {
            let (a, b) = if (j % 2 == 1) != al_reves {
                (x1, x0)
            } else {
                (x0, x1)
            };
            fila(y0 + k as f64 * separacion, a, b, k, largo, &mut tramo);
        }
        pos = *tramo.last().unwrap();
        // Las uniones entre filas pueden ser largas donde el borde es casi horizontal.
        out.push(
            subdividir(&tramo, largo)
                .into_iter()
                .map(|p| inversa * p)
                .collect(),
        );
    }
    out
}

/// Cortes de la línea horizontal `y` con los anillos, como pares (entrada, salida).
fn segmentos(anillos: &[Vec<Point>], y: f64, compensacion: f64) -> Vec<(f64, f64)> {
    let mut xs = Vec::new();
    for anillo in anillos {
        for (i, &a) in anillo.iter().enumerate() {
            let b = anillo[(i + 1) % anillo.len()];
            if (a.y <= y) != (b.y <= y) {
                xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.as_chunks::<2>()
        .0
        .iter()
        .map(|[a, b]| (a - compensacion, b + compensacion))
        .filter(|(a, b)| b - a > MINIMO)
        .collect()
}

/// Agrupa los segmentos en secciones: listas de (fila, x0, x1) de filas consecutivas.
fn secciones(filas: &[Vec<(f64, f64)>]) -> Vec<Vec<(usize, f64, f64)>> {
    let solapan = |a: (f64, f64), b: (f64, f64)| a.0 < b.1 && b.0 < a.1;
    let mut secciones: Vec<Vec<(usize, f64, f64)>> = Vec::new();
    // Para cada segmento de la fila anterior, la sección a la que pertenece.
    let mut anterior: Vec<((f64, f64), usize)> = Vec::new();
    for (k, fila) in filas.iter().enumerate() {
        let mut actual = Vec::with_capacity(fila.len());
        for &seg in fila {
            let arriba: Vec<_> = anterior.iter().filter(|(p, _)| solapan(*p, seg)).collect();
            let destino = match arriba.as_slice() {
                [(p, s)] if fila.iter().filter(|&&o| solapan(*p, o)).count() == 1 => *s,
                _ => {
                    secciones.push(vec![]);
                    secciones.len() - 1
                }
            };
            secciones[destino].push((k, seg.0, seg.1));
            actual.push((seg, destino));
        }
        anterior = actual;
    }
    secciones
}

/// Agrega las puntadas de una fila de `a` a `b`, alineadas a una grilla desfasada por fila.
fn fila(y: f64, a: f64, b: f64, k: usize, largo: f64, out: &mut Vec<Point>) {
    out.push(Point::new(a, y));
    let fase = DESFASES[k % DESFASES.len()] * largo;
    let (lo, hi) = (a.min(b), a.max(b));
    let primero = ((lo - fase) / largo).ceil() as i64;
    let ultimo = ((hi - fase) / largo).floor() as i64;
    let mut grilla: Vec<f64> = (primero..=ultimo)
        .map(|m| fase + m as f64 * largo)
        .filter(|&x| x - lo > MARGEN * largo && hi - x > MARGEN * largo)
        .collect();
    if a > b {
        grilla.reverse();
    }
    out.extend(grilla.into_iter().map(|x| Point::new(x, y)));
    out.push(Point::new(b, y));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
            .map(Point::from)
            .to_vec()
    }

    #[test]
    fn convex_shape_is_one_serpentine() {
        let t = relleno(
            &[rect(0.0, 0.0, 20.0, 10.0)],
            0.0,
            0.5,
            3.0,
            0.0,
            Point::ZERO,
        );
        assert_eq!(t.len(), 1);
        let pts = &t[0];
        assert!(pts.windows(2).all(|w| w[0].distance(w[1]) <= 3.0 + 1e-9));
        assert!(pts.iter().all(|p| (-1e-9..=20.0 + 1e-9).contains(&p.x)));
        assert!(pts.iter().all(|p| (0.0..=10.0).contains(&p.y)));
        // 20 filas: de 0,25 a 9,75.
        let mut ys: Vec<i64> = pts.iter().map(|p| (p.y * 100.0).round() as i64).collect();
        ys.dedup();
        assert_eq!(ys.len(), 20);
        assert_eq!((ys[0], ys[19]), (25, 975));
    }

    #[test]
    fn hole_stays_empty() {
        let anillos = [rect(0.0, 0.0, 30.0, 30.0), rect(10.0, 10.0, 20.0, 20.0)];
        let tramos = relleno(&anillos, 30.0, 0.4, 3.0, 0.0, Point::ZERO);
        assert!(tramos.len() >= 2, "el agujero obliga a separar secciones");
        for t in &tramos {
            for w in t.windows(2) {
                // Ninguna puntada pasa por el agujero: revisar puntos intermedios.
                for s in 0..=10 {
                    let p = w[0].lerp(w[1], s as f64 / 10.0);
                    let dentro_agujero = p.x > 10.1 && p.x < 19.9 && p.y > 10.1 && p.y < 19.9;
                    assert!(!dentro_agujero, "{p:?}");
                }
                assert!(w[0].distance(w[1]) <= 3.0 + 1e-9);
            }
            for p in t {
                assert!((-0.01..=30.01).contains(&p.x) && (-0.01..=30.01).contains(&p.y));
            }
        }
    }

    #[test]
    fn rotated_rows_follow_the_angle() {
        let t = relleno(
            &[rect(-10.0, -10.0, 10.0, 10.0)],
            90.0,
            1.0,
            3.0,
            0.0,
            Point::ZERO,
        );
        // A 90° las filas son verticales: la mayoría de las puntadas no cambia x.
        let verticales = t[0]
            .windows(2)
            .filter(|w| (w[0].x - w[1].x).abs() < 1e-9)
            .count();
        assert!(verticales * 10 > t[0].len() * 8);
    }
}
