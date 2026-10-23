use crate::geom::{Polilinea, recorrido, remuestrear};

use super::Tramo;

/// Pespunte sobre cada polilínea. El triple cose cada tramo ida, vuelta e ida.
pub fn corrido(lineas: &[Polilinea], largo: f64, triple: bool) -> Vec<Tramo> {
    lineas
        .iter()
        .map(|pl| {
            let pts = remuestrear(&recorrido(pl), largo);
            if !triple {
                return pts;
            }
            let mut out = vec![pts[0]];
            for w in pts.windows(2) {
                out.extend([w[1], w[0], w[1]]);
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;

    #[test]
    fn triple_goes_back_and_forth() {
        let pl = Polilinea {
            puntos: vec![Point::new(0.0, 0.0), Point::new(4.0, 0.0)],
            cerrada: false,
        };
        let simple = &corrido(std::slice::from_ref(&pl), 2.0, false)[0];
        assert_eq!(simple.len(), 3);
        let triple = &corrido(&[pl], 2.0, true)[0];
        assert_eq!(triple.len(), 1 + 3 * 2);
        assert_eq!(triple[2], Point::ZERO);
        assert_eq!(triple.last(), simple.last());
    }
}
