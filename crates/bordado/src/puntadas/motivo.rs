//! Motivos: un dibujo chico que se repite a lo largo del trazado.
//!
//! Cada motivo es una polilínea en coordenadas de la repetición: `u` de 0 a 1 a lo largo del
//! trazado y `v` de −1 a 1 a lo ancho (−1 = derecha, 1 = izquierda del sentido de avance).
//! Termina donde empieza la repetición siguiente, así se cose todo seguido.

use std::f64::consts::PI;

use kurbo::Point;

use crate::geom::{Arco, Polilinea, normal, recorrido, subdividir};
use crate::proyecto::Motivo;

use super::Tramo;

/// Las puntadas del motivo nunca son más largas que esto (mm).
const MAXIMO: f64 = 3.5;

/// Puntos `(u, v)` de una repetición, incluido el primero.
fn dibujo(motivo: Motivo) -> Vec<(f64, f64)> {
    let curva = |n: usize, f: &dyn Fn(f64) -> f64| -> Vec<(f64, f64)> {
        (0..=n)
            .map(|k| {
                let u = k as f64 / n as f64;
                (u, f(u))
            })
            .collect()
    };
    match motivo {
        Motivo::Onda => curva(8, &|u| (2.0 * PI * u).sin()),
        Motivo::Sierra => vec![(0.0, 0.0), (0.25, 1.0), (0.75, -1.0), (1.0, 0.0)],
        Motivo::Almenas => vec![
            (0.0, 0.0),
            (0.0, 1.0),
            (0.5, 1.0),
            (0.5, -1.0),
            (1.0, -1.0),
            (1.0, 0.0),
        ],
        // Arriba y abajo, y vuelta al eje por el centro del rombo.
        Motivo::Rombos => vec![
            (0.0, 0.0),
            (0.5, 1.0),
            (1.0, 0.0),
            (0.5, -1.0),
            (0.0, 0.0),
            (1.0, 0.0),
        ],
        Motivo::Festones => curva(6, &|u| (PI * u).sin()),
        // La X y la barra de arriba que la une con la siguiente.
        Motivo::Cruces => vec![(0.0, -1.0), (1.0, 1.0), (0.0, 1.0), (1.0, -1.0)],
    }
}

/// Repite el motivo sobre cada polilínea; el largo de la repetición se ajusta para que entre
/// un número entero de veces.
pub fn motivo(lineas: &[Polilinea], motivo: Motivo, largo: f64, ancho: f64) -> Vec<Tramo> {
    let unidad = dibujo(motivo);
    lineas
        .iter()
        .map(|pl| {
            let arco = Arco::new(&recorrido(pl));
            let total = arco.largo();
            let n = (total / largo).round().max(1.0) as usize;
            let paso = total / n as f64;
            let mut out: Vec<Point> = Vec::new();
            for k in 0..n {
                // La primera repetición pone su primer punto; las otras lo comparten.
                let desde = usize::from(k > 0);
                for &(u, v) in &unidad[desde..] {
                    let (p, t) = arco.en((k as f64 + u) * paso);
                    out.push(p + normal(t) * (v * ancho / 2.0));
                }
            }
            subdividir(&out, MAXIMO)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_motif_is_continuous() {
        for m in Motivo::TODOS {
            let d = dibujo(m);
            let (a, z) = (d[0], *d.last().unwrap());
            assert!(
                (z.0 - a.0 - 1.0).abs() < 1e-9 && (z.1 - a.1).abs() < 1e-9,
                "{m:?}"
            );
            assert!(
                d.iter()
                    .all(|&(u, v)| (0.0..=1.0).contains(&u) && v.abs() <= 1.0)
            );
        }
    }

    #[test]
    fn repeats_along_the_whole_line_within_width() {
        let pl = Polilinea {
            puntos: vec![Point::ZERO, Point::new(21.0, 0.0)],
            cerrada: false,
        };
        for m in Motivo::TODOS {
            let t = &motivo(std::slice::from_ref(&pl), m, 4.0, 3.0)[0];
            let (x0, x1) = t
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
            assert!(x0.abs() < 1e-9 && (x1 - 21.0).abs() < 1e-9, "{m:?}");
            assert!(t.iter().all(|p| p.y.abs() <= 1.5 + 1e-9));
            assert!(t.windows(2).all(|w| w[0].distance(w[1]) <= MAXIMO + 1e-9));
        }
    }
}
