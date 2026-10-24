//! Geometría en milímetros: curvas a polilíneas y remuestreo.

use kurbo::{BezPath, PathEl, Point, Vec2};

/// Error máximo al aproximar curvas con segmentos, en mm.
pub const TOLERANCIA: f64 = 0.02;
/// Giro mínimo (grados) para que un vértice cuente como esquina y reciba una puntada.
const ESQUINA: f64 = 25.0;

/// Una polilínea; si es cerrada, el último punto no repite el primero.
#[derive(Debug, Clone, PartialEq)]
pub struct Polilinea {
    pub puntos: Vec<Point>,
    pub cerrada: bool,
}

/// Aproxima cada subtrazado con segmentos. Descarta puntos repetidos y subtrazados vacíos.
pub fn polilineas(path: &BezPath) -> Vec<Polilinea> {
    let mut out = Vec::new();
    let mut actual = Polilinea {
        puntos: vec![],
        cerrada: false,
    };
    let mut terminar = |pl: &mut Polilinea, cerrada: bool| {
        let mut pl = std::mem::replace(
            pl,
            Polilinea {
                puntos: vec![],
                cerrada: false,
            },
        );
        pl.cerrada = cerrada;
        if cerrada && pl.puntos.len() > 1 && pl.puntos.first() == pl.puntos.last() {
            pl.puntos.pop();
        }
        if pl.puntos.len() > 1 {
            out.push(pl);
        }
    };
    kurbo::flatten(path.iter(), TOLERANCIA, |el| match el {
        PathEl::MoveTo(p) => {
            terminar(&mut actual, false);
            actual.puntos.push(p);
        }
        PathEl::LineTo(p) => {
            if actual.puntos.last().is_none_or(|q| q.distance(p) > 1e-6) {
                actual.puntos.push(p);
            }
        }
        PathEl::ClosePath => terminar(&mut actual, true),
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => unreachable!("flatten solo da segmentos"),
    });
    terminar(&mut actual, false);
    out
}

/// Separa un trazado en sus subtrazados (uno por cada `M`). Descarta los que no dibujan nada.
pub fn subtrazados(path: &BezPath) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = Vec::new();
    for el in path.elements() {
        if matches!(el, PathEl::MoveTo(_)) || out.is_empty() {
            out.push(BezPath::new());
        }
        out.last_mut().unwrap().push(*el);
    }
    out.retain(|d| d.elements().len() >= 2);
    out
}

/// Puntos de la polilínea en orden de recorrido; si es cerrada, termina repitiendo el primero.
pub fn recorrido(pl: &Polilinea) -> Vec<Point> {
    let mut v = pl.puntos.clone();
    if pl.cerrada {
        v.push(v[0]);
    }
    v
}

/// Distribuye puntos sobre el recorrido con separación lo más cercana posible a `paso`
/// sin superarla. Las esquinas siempre reciben un punto; las curvas suaves no.
pub fn remuestrear(puntos: &[Point], paso: f64) -> Vec<Point> {
    let Some(&first) = puntos.first() else {
        return vec![];
    };
    let mut out = vec![first];
    let mut tramo = vec![first];
    for i in 1..puntos.len() {
        tramo.push(puntos[i]);
        let esquina = puntos
            .get(i + 1)
            .is_none_or(|&next| giro(puntos[i - 1], puntos[i], next) > ESQUINA);
        if esquina {
            dividir(&tramo, paso, &mut out);
            tramo = vec![puntos[i]];
        }
    }
    out
}

/// Ángulo (grados) entre a→b y b→c.
fn giro(a: Point, b: Point, c: Point) -> f64 {
    let (u, v) = (b - a, c - b);
    if u.hypot() < 1e-9 || v.hypot() < 1e-9 {
        return 0.0;
    }
    u.cross(v).atan2(u.dot(v)).abs().to_degrees()
}

/// Agrega a `out` puntos equiespaciados sobre `tramo` (sin el primero, que ya está).
fn dividir(tramo: &[Point], paso: f64, out: &mut Vec<Point>) {
    let largos: Vec<f64> = tramo.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f64 = largos.iter().sum();
    if total < 1e-9 {
        return;
    }
    let n = (total / paso).ceil().max(1.0) as usize;
    let mut seg = 0;
    let mut acumulado = 0.0;
    for k in 1..=n {
        let objetivo = total * k as f64 / n as f64;
        while seg + 1 < largos.len() && acumulado + largos[seg] < objetivo {
            acumulado += largos[seg];
            seg += 1;
        }
        let t = if largos[seg] > 0.0 {
            ((objetivo - acumulado) / largos[seg]).clamp(0.0, 1.0)
        } else {
            1.0
        };
        out.push(tramo[seg].lerp(tramo[seg + 1], t));
    }
}

/// Parte los segmentos más largos que `max` en partes iguales.
pub fn subdividir(puntos: &[Point], max: f64) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(puntos.len());
    for &p in puntos {
        if let Some(&q) = out.last() {
            let n = (q.distance(p) / max).ceil() as usize;
            for k in 1..n {
                out.push(q.lerp(p, k as f64 / n as f64));
            }
        }
        out.push(p);
    }
    out
}

/// Vector unitario, o cero si es nulo.
pub fn unitario(v: Vec2) -> Vec2 {
    let n = v.hypot();
    if n > 1e-12 { v / n } else { Vec2::ZERO }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_and_open_subpaths() {
        let path = BezPath::from_svg("M0,0 L10,0 L10,10 Z M20,0 L30,0").unwrap();
        let pls = polilineas(&path);
        assert_eq!(pls.len(), 2);
        assert!(pls[0].cerrada && pls[0].puntos.len() == 3);
        assert!(!pls[1].cerrada && pls[1].puntos.len() == 2);
        assert_eq!(recorrido(&pls[0]).last(), Some(&Point::ZERO));
    }

    #[test]
    fn splits_subpaths() {
        let path = BezPath::from_svg("M0,0 L10,0 L10,10 Z M2,2 L3,2 L3,3 Z M7,7").unwrap();
        let partes = subtrazados(&path);
        assert_eq!(partes.len(), 2, "el M suelto no cuenta");
        assert_eq!(partes[0].to_svg(), "M0,0 L10,0 L10,10 Z");
        assert_eq!(partes[1].to_svg(), "M2,2 L3,2 L3,3 Z");
    }

    #[test]
    fn resample_keeps_corners_and_max_step() {
        // Cuadrado de 10 mm con paso 3: cada lado en 4 partes de 2,5.
        let sq = [(0., 0.), (10., 0.), (10., 10.), (0., 10.), (0., 0.)].map(Point::from);
        let pts = remuestrear(&sq, 3.0);
        assert_eq!(pts.len(), 17);
        for c in &sq {
            assert!(pts.iter().any(|p| p.distance(*c) < 1e-9));
        }
        assert!(pts.windows(2).all(|w| w[0].distance(w[1]) <= 3.0 + 1e-9));

        // Un círculo aproximado no tiene esquinas: los puntos quedan parejos.
        let circle = BezPath::from_svg("M10,0 A10,10 0 1 1 -10,0 A10,10 0 1 1 10,0 Z").unwrap();
        let pts = remuestrear(&recorrido(&polilineas(&circle)[0]), 2.0);
        let d: Vec<f64> = pts.windows(2).map(|w| w[0].distance(w[1])).collect();
        let (min, max) = d
            .iter()
            .fold((f64::MAX, 0f64), |(a, b), &x| (a.min(x), b.max(x)));
        assert!(max <= 2.0 + 1e-6 && min > 1.8, "{min}..{max}");
    }
}
