//! Puntadas decorativas de línea: manual, punto atrás, tallo y festón. Todas siguen el
//! trazado; en las zonas, el borde.

use kurbo::Point;

use crate::geom::{Arco, Polilinea, normal, recorrido, remuestrear, subdividir};
use crate::proyecto::Lado;

use super::Tramo;

/// Una puntada nunca es más larga que esto (mm): las más largas se enganchan.
const MAXIMO: f64 = 7.0;

/// Una puntada en cada nodo. Los tramos muy largos se parten.
pub fn manual(lineas: &[Polilinea]) -> Vec<Tramo> {
    lineas
        .iter()
        .map(|pl| subdividir(&recorrido(pl), MAXIMO))
        .collect()
}

/// Punto atrás: de cada punto se salta dos adelante y se vuelve uno (0 → 2 → 1 → 3 → 2 …),
/// así cada tramo queda cubierto por dos puntadas que se montan.
pub fn punto_atras(lineas: &[Polilinea], largo: f64) -> Vec<Tramo> {
    lineas
        .iter()
        .map(|pl| {
            let q = remuestrear(&recorrido(pl), largo);
            if q.len() < 3 {
                return q;
            }
            let mut out = vec![q[0]];
            for i in 0..q.len() - 2 {
                out.extend([q[i + 2], q[i + 1]]);
            }
            out.push(*q.last().unwrap());
            out
        })
        .collect()
}

/// Punto tallo: puntadas inclinadas de 1,5 pasos que se montan. Cada una va de un lado de la
/// línea al otro (separados `ancho`) y la vuelta queda tapada por la siguiente.
pub fn tallo(lineas: &[Polilinea], largo: f64, ancho: f64) -> Vec<Tramo> {
    lineas
        .iter()
        .map(|pl| {
            let arco = Arco::new(&recorrido(pl));
            let total = arco.largo();
            let n = (total / largo).ceil().max(1.0) as usize;
            let paso = total / n as f64;
            let lado = |s: f64, signo: f64| {
                let (p, t) = arco.en(s);
                p + normal(t) * (signo * ancho / 2.0)
            };
            let mut out = vec![arco.en(0.0).0];
            for k in 0..n {
                let s = k as f64 * paso;
                out.push(lado((s + 1.5 * paso).min(total), 1.0));
                if k + 1 < n {
                    out.push(lado(s + paso, -1.0));
                }
            }
            out.push(arco.en(total).0);
            out
        })
        .collect()
}

/// Festón: pespunte por la línea y cada `separacion` un diente de `ancho` hacia el lado
/// pedido (ida y vuelta por el mismo agujero).
pub fn feston(lineas: &[Polilinea], ancho: f64, separacion: f64, lado: Lado) -> Vec<Tramo> {
    lineas
        .iter()
        .map(|pl| {
            let arco = Arco::new(&recorrido(pl));
            let total = arco.largo();
            let n = (total / separacion).round().max(1.0) as usize;
            let paso = total / n as f64;
            let mut out: Vec<Point> = Vec::new();
            for k in 0..=n {
                let (p, t) = arco.en(k as f64 * paso);
                let signo = match lado {
                    Lado::Izquierda => 1.0,
                    Lado::Derecha => -1.0,
                    Lado::Ambos if k % 2 == 0 => 1.0,
                    Lado::Ambos => -1.0,
                };
                out.extend([p, p + normal(t) * (signo * ancho), p]);
            }
            subdividir(&out, MAXIMO)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recta(largo: f64) -> Polilinea {
        Polilinea {
            puntos: vec![Point::ZERO, Point::new(largo, 0.0)],
            cerrada: false,
        }
    }

    #[test]
    fn manual_keeps_nodes_and_splits_long_ones() {
        let pl = Polilinea {
            puntos: vec![Point::ZERO, Point::new(1.0, 1.0), Point::new(15.0, 1.0)],
            cerrada: false,
        };
        let t = &manual(&[pl])[0];
        assert_eq!(t[..2], [Point::ZERO, Point::new(1.0, 1.0)]);
        assert_eq!(t.len(), 2 + 2, "14 mm en dos puntadas de 7");
    }

    #[test]
    fn backstitch_covers_each_step_twice() {
        let t = &punto_atras(&[recta(6.0)], 2.0)[0];
        let xs: Vec<f64> = t.iter().map(|p| p.x).collect();
        assert_eq!(xs, vec![0.0, 4.0, 2.0, 6.0, 4.0, 6.0]);
    }

    #[test]
    fn stem_alternates_sides_and_ends_on_the_line() {
        let t = &tallo(&[recta(9.0)], 3.0, 1.0)[0];
        assert_eq!(t.first(), Some(&Point::ZERO));
        assert_eq!(t.last(), Some(&Point::new(9.0, 0.0)));
        for (i, p) in t[1..t.len() - 1].iter().enumerate() {
            let y = if i % 2 == 0 { -0.5 } else { 0.5 };
            assert!((p.y - y).abs() < 1e-9, "{i}: {p:?}");
        }
        assert!(t.windows(2).all(|w| w[0].distance(w[1]) < 5.0));
    }

    #[test]
    fn blanket_teeth_on_the_chosen_side() {
        for (lado, signo) in [(Lado::Izquierda, -1.0), (Lado::Derecha, 1.0)] {
            let t = &feston(&[recta(10.0)], 3.0, 2.0, lado)[0];
            let dientes: Vec<_> = t.iter().filter(|p| p.y.abs() > 1e-9).collect();
            assert_eq!(dientes.len(), 6);
            assert!(dientes.iter().all(|p| (p.y - signo * 3.0).abs() < 1e-9));
        }
        let t = &feston(&[recta(10.0)], 3.0, 2.0, Lado::Ambos)[0];
        let ys: Vec<f64> = t.iter().map(|p| p.y).filter(|y| y.abs() > 1e-9).collect();
        assert!(ys.windows(2).all(|w| w[0] == -w[1]));
    }
}
