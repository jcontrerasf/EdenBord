//! Relleno concéntrico: filas que siguen el borde de la zona, cada una metida `separacion`
//! más adentro que la anterior (con los agujeros pasa lo mismo, hacia afuera de ellos).

use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::OutlineStyle;
use kurbo::Point;

use crate::geom::remuestrear;

use super::Tramo;

/// Anillos con un perímetro menor que esto (mm) no se cosen.
const MINIMO: f64 = 1.0;
/// Tope de filas, por si la zona es enorme y la separación mínima.
const MAX_FILAS: usize = 2000;

/// Cose los anillos de afuera hacia adentro. Pasa de un anillo al siguiente con una puntada
/// si están cerca; si no, empieza un tramo nuevo (salto).
pub fn concentrico(
    anillos: &[Vec<Point>],
    separacion: f64,
    largo: f64,
    desde: Point,
) -> Vec<Tramo> {
    if separacion <= 0.0 || largo <= 0.0 {
        return vec![];
    }
    let contornos: Vec<Vec<[f64; 2]>> = anillos
        .iter()
        .filter(|a| a.len() > 2)
        .map(|a| a.iter().map(|p| [p.x, p.y]).collect())
        .collect();
    // Orienta los contornos como pide `outline` (regla par-impar, como el resto).
    let base = contornos.simplify_shape(FillRule::EvenOdd);

    let mut out: Vec<Tramo> = Vec::new();
    let mut pos = desde;
    for k in 0..MAX_FILAS {
        let metido = -(separacion / 2.0 + k as f64 * separacion);
        let nivel: Vec<Vec<Point>> = base
            .outline(&OutlineStyle::new(metido))
            .into_iter()
            .flatten()
            .map(|c| {
                c.into_iter()
                    .map(|[x, y]| Point::new(x, y))
                    .collect::<Vec<_>>()
            })
            .filter(|c| perimetro(c) >= MINIMO)
            .collect();
        if nivel.is_empty() {
            break;
        }
        let mut pendientes = nivel;
        while !pendientes.is_empty() {
            // El anillo y el vértice más cercanos a la aguja.
            let (i, j) = (0..pendientes.len())
                .flat_map(|i| (0..pendientes[i].len()).map(move |j| (i, j)))
                .min_by(|&(a, b), &(c, d)| {
                    pos.distance(pendientes[a][b])
                        .total_cmp(&pos.distance(pendientes[c][d]))
                })
                .unwrap();
            let anillo = pendientes.swap_remove(i);
            let mut vuelta: Vec<Point> = anillo[j..].iter().chain(&anillo[..=j]).copied().collect();
            if out.is_empty() || pos.distance(vuelta[0]) > 2.0 * separacion.max(0.5) {
                out.push(vec![]);
            } else {
                // Sigue en el mismo tramo: la unión es una puntada corta.
                vuelta.insert(0, pos);
            }
            let tramo = out.last_mut().unwrap();
            let puntos = remuestrear(&vuelta, largo);
            let saltar = usize::from(!tramo.is_empty());
            tramo.extend_from_slice(&puntos[saltar..]);
            pos = *tramo.last().unwrap();
        }
    }
    out
}

fn perimetro(c: &[Point]) -> f64 {
    (0..c.len())
        .map(|i| c[i].distance(c[(i + 1) % c.len()]))
        .sum()
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
    fn rings_go_inwards_and_stay_inside() {
        let t = concentrico(&[rect(0.0, 0.0, 20.0, 10.0)], 1.0, 2.0, Point::ZERO);
        assert_eq!(t.len(), 1, "anillos cercanos: un solo tramo");
        let pts = &t[0];
        assert!(
            pts.iter()
                .all(|p| (0.4..=19.6).contains(&p.x) && (0.4..=9.6).contains(&p.y))
        );
        assert!(pts.windows(2).all(|w| w[0].distance(w[1]) <= 2.0 + 1e-6));
        // Llega cerca del centro.
        assert!(
            pts.iter()
                .any(|p| (p.y - 5.0).abs() < 1.0 && (p.x - 10.0).abs() < 5.0)
        );
    }

    #[test]
    fn hole_stays_empty() {
        let anillos = [rect(0.0, 0.0, 30.0, 30.0), rect(10.0, 10.0, 20.0, 20.0)];
        let t = concentrico(&anillos, 0.8, 2.5, Point::ZERO);
        for p in t.iter().flatten() {
            let dentro = p.x > 10.2 && p.x < 19.8 && p.y > 10.2 && p.y < 19.8;
            assert!(!dentro, "{p:?}");
        }
    }
}
