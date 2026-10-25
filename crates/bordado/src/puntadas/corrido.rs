use crate::geom::{Polilinea, recorrido, remuestrear};

use super::Tramo;

/// Pespunte sobre cada polilínea. Con `pasadas` = 3, 5… cada puntada se cose ida, vuelta e
/// ida (las veces que haga falta): línea más gruesa. Un número par se toma como el impar
/// siguiente, para terminar siempre adelante.
pub fn corrido(lineas: &[Polilinea], largo: f64, pasadas: u32) -> Vec<Tramo> {
    let vueltas = pasadas.max(1) / 2;
    lineas
        .iter()
        .map(|pl| {
            let pts = remuestrear(&recorrido(pl), largo);
            if vueltas == 0 {
                return pts;
            }
            let mut out = vec![pts[0]];
            for w in pts.windows(2) {
                out.push(w[1]);
                for _ in 0..vueltas {
                    out.extend([w[0], w[1]]);
                }
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
    fn passes_go_back_and_forth() {
        let pl = Polilinea {
            puntos: vec![Point::new(0.0, 0.0), Point::new(4.0, 0.0)],
            cerrada: false,
        };
        let simple = &corrido(std::slice::from_ref(&pl), 2.0, 1)[0];
        assert_eq!(simple.len(), 3);
        let triple = &corrido(std::slice::from_ref(&pl), 2.0, 3)[0];
        assert_eq!(triple.len(), 1 + 3 * 2);
        assert_eq!(triple[2], Point::ZERO);
        assert_eq!(triple.last(), simple.last());
        let bean = &corrido(std::slice::from_ref(&pl), 2.0, 5)[0];
        assert_eq!(bean.len(), 1 + 5 * 2);
        assert_eq!(corrido(&[pl], 2.0, 4)[0], *bean, "par = impar siguiente");
    }
}
