//! Edición de nodos de un trazado: el `BezPath` se pasa a nodos con manijas de entrada y
//! salida (todo cúbico), se edita y se vuelve a armar.

use kurbo::{BezPath, CubicBez, ParamCurve, ParamCurveNearest, PathEl, Point};

/// Distancia bajo la cual dos puntos se consideran el mismo (mm).
const EPS: f64 = 1e-6;

/// Un nodo con sus manijas en coordenadas absolutas. Manija = punto: sin curva de ese lado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nodo {
    pub p: Point,
    pub entrada: Point,
    pub salida: Point,
}

impl Nodo {
    fn esquina(p: Point) -> Self {
        Nodo {
            p,
            entrada: p,
            salida: p,
        }
    }

    pub fn es_esquina(&self) -> bool {
        self.entrada.distance(self.p) < EPS && self.salida.distance(self.p) < EPS
    }
}

/// Un subtrazado: nodos en orden y si se cierra del último al primero.
#[derive(Debug, Clone, PartialEq)]
pub struct Sub {
    pub nodos: Vec<Nodo>,
    pub cerrado: bool,
}

impl Sub {
    /// Cantidad de segmentos.
    pub fn segmentos(&self) -> usize {
        let n = self.nodos.len();
        if self.cerrado { n } else { n.saturating_sub(1) }
    }

    /// Segmento `k`: del nodo `k` al siguiente.
    pub fn segmento(&self, k: usize) -> CubicBez {
        let a = self.nodos[k];
        let b = self.nodos[(k + 1) % self.nodos.len()];
        CubicBez::new(a.p, a.salida, b.entrada, b.p)
    }

    fn minimo(&self) -> usize {
        if self.cerrado { 3 } else { 2 }
    }
}

/// Identifica un nodo: subtrazado y posición.
pub type Id = (usize, usize);

/// Qué manija de un nodo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Entrada,
    Salida,
}

/// Pasa un trazado a nodos. Las cuadráticas se elevan a cúbicas; un subtrazado cerrado cuyo
/// último punto coincide con el primero se junta en un solo nodo.
pub fn desde_trazado(path: &BezPath) -> Vec<Sub> {
    let mut subs: Vec<Sub> = Vec::new();
    let mut actual: Option<Sub> = None;
    let terminar = |s: Option<Sub>, subs: &mut Vec<Sub>| {
        if let Some(s) = s
            && s.nodos.len() > 1
        {
            subs.push(s);
        }
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                terminar(actual.take(), &mut subs);
                actual = Some(Sub {
                    nodos: vec![Nodo::esquina(p)],
                    cerrado: false,
                });
            }
            PathEl::LineTo(p) => {
                if let Some(s) = &mut actual {
                    s.nodos.push(Nodo::esquina(p));
                }
            }
            PathEl::QuadTo(c, p) => {
                if let Some(s) = &mut actual {
                    let a = s.nodos.last().unwrap().p;
                    s.nodos.last_mut().unwrap().salida = a + (c - a) * (2.0 / 3.0);
                    s.nodos.push(Nodo {
                        p,
                        entrada: p + (c - p) * (2.0 / 3.0),
                        salida: p,
                    });
                }
            }
            PathEl::CurveTo(c1, c2, p) => {
                if let Some(s) = &mut actual {
                    s.nodos.last_mut().unwrap().salida = c1;
                    s.nodos.push(Nodo {
                        p,
                        entrada: c2,
                        salida: p,
                    });
                }
            }
            PathEl::ClosePath => {
                if let Some(mut s) = actual.take() {
                    s.cerrado = true;
                    if s.nodos.len() > 1 && s.nodos.last().unwrap().p.distance(s.nodos[0].p) < EPS {
                        let ultimo = s.nodos.pop().unwrap();
                        s.nodos[0].entrada = ultimo.entrada;
                    }
                    terminar(Some(s), &mut subs);
                }
            }
        }
    }
    terminar(actual, &mut subs);
    subs
}

/// Arma el trazado. Los segmentos sin manijas salen como líneas; el cierre recto queda
/// implícito en el `Z`.
pub fn a_trazado(subs: &[Sub]) -> BezPath {
    let mut path = BezPath::new();
    for s in subs {
        let Some(first) = s.nodos.first() else {
            continue;
        };
        path.move_to(first.p);
        for k in 0..s.segmentos() {
            let c = s.segmento(k);
            let recto = c.p1.distance(c.p0) < EPS && c.p2.distance(c.p3) < EPS;
            let cierre = s.cerrado && k + 1 == s.nodos.len();
            match (recto, cierre) {
                (true, true) => {}
                (true, false) => path.line_to(c.p3),
                (false, _) => path.curve_to(c.p1, c.p2, c.p3),
            }
        }
        if s.cerrado {
            path.close_path();
        }
    }
    path
}

/// Mueve los nodos (con sus manijas).
pub fn mover(subs: &mut [Sub], ids: &[Id], d: kurbo::Vec2) {
    for &(s, k) in ids {
        if let Some(n) = subs.get_mut(s).and_then(|s| s.nodos.get_mut(k)) {
            n.p += d;
            n.entrada += d;
            n.salida += d;
        }
    }
}

/// Pone una manija en `p`.
pub fn mover_manija(subs: &mut [Sub], (s, k): Id, lado: Lado, p: Point) {
    if let Some(n) = subs.get_mut(s).and_then(|s| s.nodos.get_mut(k)) {
        match lado {
            Lado::Entrada => n.entrada = p,
            Lado::Salida => n.salida = p,
        }
    }
}

/// Parte el segmento `k` del subtrazado `s` en `t` con un nodo nuevo; devuelve su id. Un
/// segmento recto sigue recto.
pub fn insertar(subs: &mut [Sub], s: usize, k: usize, t: f64) -> Id {
    let sub = &mut subs[s];
    let n = sub.nodos.len();
    let c = sub.segmento(k);
    let siguiente = (k + 1) % n;
    let nuevo = if c.p1.distance(c.p0) < EPS && c.p2.distance(c.p3) < EPS {
        Nodo::esquina(c.p0.lerp(c.p3, t))
    } else {
        let (a, b) = (c.subsegment(0.0..t), c.subsegment(t..1.0));
        sub.nodos[k].salida = a.p1;
        sub.nodos[siguiente].entrada = b.p2;
        Nodo {
            p: a.p3,
            entrada: a.p2,
            salida: b.p1,
        }
    };
    sub.nodos.insert(k + 1, nuevo);
    (s, k + 1)
}

/// Borra los nodos; un subtrazado no baja de 3 nodos (cerrado) o 2 (abierto). Los vecinos
/// conservan sus manijas, así que el hueco se une con una curva. Devuelve si borró algo.
pub fn borrar(subs: &mut [Sub], ids: &[Id]) -> bool {
    let mut borro = false;
    for (s, sub) in subs.iter_mut().enumerate() {
        let mut quitar: Vec<usize> = ids.iter().filter(|i| i.0 == s).map(|i| i.1).collect();
        quitar.sort_unstable();
        quitar.dedup();
        for k in quitar.into_iter().rev() {
            if k < sub.nodos.len() && sub.nodos.len() > sub.minimo() {
                sub.nodos.remove(k);
                borro = true;
            }
        }
    }
    borro
}

/// Esquina ↔ curva: una esquina toma manijas en la dirección de sus vecinos; una curva las
/// pierde.
pub fn alternar_curva(subs: &mut [Sub], ids: &[Id]) {
    for &(s, k) in ids {
        let Some(sub) = subs.get_mut(s) else {
            continue;
        };
        let n = sub.nodos.len();
        if k >= n {
            continue;
        }
        let nodo = sub.nodos[k];
        if !nodo.es_esquina() {
            sub.nodos[k] = Nodo::esquina(nodo.p);
            continue;
        }
        let prev = (k > 0 || sub.cerrado).then(|| sub.nodos[(k + n - 1) % n].p);
        let next = (k + 1 < n || sub.cerrado).then(|| sub.nodos[(k + 1) % n].p);
        let dir = match (prev, next) {
            (Some(a), Some(b)) => b - a,
            (Some(a), None) => nodo.p - a,
            (None, Some(b)) => b - nodo.p,
            (None, None) => continue,
        };
        if dir.hypot() < EPS {
            continue;
        }
        let dir = dir.normalize();
        let largo = |q: Option<Point>| q.map_or(0.0, |q| q.distance(nodo.p) / 3.0);
        sub.nodos[k].entrada = nodo.p - dir * largo(prev);
        sub.nodos[k].salida = nodo.p + dir * largo(next);
    }
}

/// Segmento más cercano a `p`: (subtrazado, segmento, t, distancia).
pub fn segmento_cercano(subs: &[Sub], p: Point) -> Option<(usize, usize, f64, f64)> {
    let mut mejor: Option<(usize, usize, f64, f64)> = None;
    for (s, sub) in subs.iter().enumerate() {
        for k in 0..sub.segmentos() {
            let n = sub.segmento(k).nearest(p, 1e-6);
            let d = n.distance_sq.sqrt();
            if mejor.is_none_or(|m| d < m.3) {
                mejor = Some((s, k, n.t, d));
            }
        }
    }
    mejor
}

/// Todos los ids de los nodos.
pub fn todos(subs: &[Sub]) -> impl Iterator<Item = Id> + '_ {
    subs.iter()
        .enumerate()
        .flat_map(|(s, sub)| (0..sub.nodos.len()).map(move |k| (s, k)))
}

pub fn nodo(subs: &[Sub], (s, k): Id) -> Option<&Nodo> {
    subs.get(s)?.nodos.get(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(d: &str) -> BezPath {
        BezPath::from_svg(d).unwrap()
    }

    #[test]
    fn roundtrip_keeps_lines_curves_and_closing() {
        for d in [
            "M0,0 L10,0 L10,10 Z",
            "M0,0 L10,0 C12,2 12,8 10,10 L0,10 Z M3,3 L5,3 L5,5 Z",
            "M0,0 C1,1 2,1 3,0",
        ] {
            let subs = desde_trazado(&path(d));
            assert_eq!(a_trazado(&subs), path(d), "{d}");
        }
        // Cerrado con el último punto repetido: un solo nodo en el inicio.
        let subs = desde_trazado(&path("M0,0 L10,0 L10,10 L0,0 Z"));
        assert_eq!(subs[0].nodos.len(), 3);
        assert_eq!(a_trazado(&subs), path("M0,0 L10,0 L10,10 Z"));
        // Una cuadrática se eleva a cúbica con la misma forma.
        let q = path("M0,0 Q5,10 10,0");
        let c = a_trazado(&desde_trazado(&q));
        let mid = |p: &BezPath| p.segments().next().unwrap().eval(0.5);
        assert!(mid(&q).distance(mid(&c)) < 1e-9);
    }

    #[test]
    fn move_insert_delete_and_toggle() {
        let mut subs = desde_trazado(&path("M0,0 L10,0 L10,10 L0,10 Z"));
        mover(&mut subs, &[(0, 2)], kurbo::Vec2::new(5.0, 5.0));
        assert_eq!(subs[0].nodos[2].p, Point::new(15.0, 15.0));

        // Insertar en una recta deja una esquina en la recta.
        let id = insertar(&mut subs, 0, 0, 0.5);
        assert_eq!(id, (0, 1));
        assert_eq!(subs[0].nodos[1], Nodo::esquina(Point::new(5.0, 0.0)));
        assert_eq!(subs[0].nodos.len(), 5);

        // Insertar en una curva no cambia su forma.
        let mut curva = desde_trazado(&path("M0,0 C0,10 10,10 10,0"));
        let antes = curva[0].segmento(0).eval(0.25);
        insertar(&mut curva, 0, 0, 0.5);
        assert!(curva[0].segmento(0).eval(0.5).distance(antes) < 1e-9);

        // Borrar respeta el mínimo de un cerrado (3).
        assert!(borrar(&mut subs, &[(0, 0), (0, 1)]));
        assert_eq!(subs[0].nodos.len(), 3);
        assert!(!borrar(&mut subs, &[(0, 0)]));

        alternar_curva(&mut subs, &[(0, 1)]);
        assert!(!subs[0].nodos[1].es_esquina());
        alternar_curva(&mut subs, &[(0, 1)]);
        assert!(subs[0].nodos[1].es_esquina());
    }

    #[test]
    fn nearest_segment() {
        let subs = desde_trazado(&path("M0,0 L10,0 L10,10"));
        let (s, k, t, d) = segmento_cercano(&subs, Point::new(10.5, 5.0)).unwrap();
        assert_eq!((s, k), (0, 1));
        assert!((t - 0.5).abs() < 1e-6 && (d - 0.5).abs() < 1e-6);
    }
}
