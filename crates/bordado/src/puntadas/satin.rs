use kurbo::Vec2;

use crate::geom::{Polilinea, recorrido, remuestrear, unitario};

use super::Tramo;

/// Largo de puntada del pespunte de base.
const LARGO_BASE: f64 = 2.0;

/// Columna en zigzag de ancho fijo centrada en cada polilínea. Con base, primero cose un
/// pespunte por el centro y vuelve con el satín, así termina donde empezó.
pub fn satin(lineas: &[Polilinea], ancho: f64, densidad: f64, base: bool) -> Vec<Tramo> {
    lineas
        .iter()
        .map(|pl| {
            let centro = remuestrear(&recorrido(pl), densidad / 2.0);
            let n = centro.len();
            let mut zigzag: Vec<_> = (0..n)
                .map(|i| {
                    let (a, b) = if pl.cerrada && (i == 0 || i == n - 1) {
                        // El primero y el último son el mismo punto: tangente a través del cierre.
                        (centro[n - 2], centro[1])
                    } else {
                        (centro[i.saturating_sub(1)], centro[(i + 1).min(n - 1)])
                    };
                    let t = unitario(b - a);
                    let normal = Vec2::new(-t.y, t.x) * (ancho / 2.0);
                    if i % 2 == 0 {
                        centro[i] + normal
                    } else {
                        centro[i] - normal
                    }
                })
                .collect();
            if !base {
                return zigzag;
            }
            let mut out = remuestrear(&centro, LARGO_BASE);
            zigzag.reverse();
            out.extend(zigzag);
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;

    #[test]
    fn zigzag_alternates_sides() {
        let pl = Polilinea {
            puntos: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            cerrada: false,
        };
        let t = &satin(std::slice::from_ref(&pl), 3.0, 0.5, false)[0];
        assert_eq!(t.len(), 41);
        for (i, p) in t.iter().enumerate() {
            let lado = if i % 2 == 0 { 1.5 } else { -1.5 };
            assert!((p.y - lado).abs() < 1e-9);
        }
        let con_base = &satin(&[pl], 3.0, 0.5, true)[0];
        assert_eq!(con_base.len(), 6 + 41);
        assert_eq!(con_base[5], Point::new(10.0, 0.0));
    }
}
