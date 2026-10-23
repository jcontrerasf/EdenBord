use crate::geom::unitario;

use super::Tramo;

/// Largo de las puntadas de remate, en mm.
const LARGO: f64 = 0.6;

/// Agrega remates al principio y al final del tramo: ida y vuelta corta sobre la primera y la
/// última puntada, para que el hilo no se suelte al saltar o cortar.
pub fn con_remates(tramo: &[kurbo::Point]) -> Tramo {
    let (Some(&a), Some(&z)) = (tramo.first(), tramo.last()) else {
        return vec![];
    };
    let Some(&b) = tramo.iter().find(|p| p.distance(a) > 1e-6) else {
        return vec![a];
    };
    let y = *tramo
        .iter()
        .rev()
        .find(|p| p.distance(z) > 1e-6)
        .unwrap_or(&a);
    let ida = a + unitario(b - a) * LARGO.min(a.distance(b));
    let vuelta = z + unitario(y - z) * LARGO.min(z.distance(y));
    let mut out = Vec::with_capacity(tramo.len() + 4);
    out.extend([a, ida, a]);
    out.extend_from_slice(&tramo[1..]);
    out.extend([vuelta, z]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;

    #[test]
    fn locks_at_both_ends() {
        let t = [Point::new(0.0, 0.0), Point::new(3.0, 0.0)];
        let r = con_remates(&t);
        let xs: Vec<f64> = r.iter().map(|p| (p.x * 10.0).round() / 10.0).collect();
        assert_eq!(xs, vec![0.0, 0.6, 0.0, 3.0, 2.4, 3.0]);
    }
}
