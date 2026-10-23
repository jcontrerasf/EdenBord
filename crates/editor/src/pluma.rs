//! Herramienta pluma para calcar: clic = vértice en esquina, arrastrar = vértice con curva.

use kurbo::{BezPath, Point, Vec2};

/// Un vértice con su manija de salida (la de entrada es la opuesta). Manija nula = esquina.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nodo {
    pub p: Point,
    pub h: Vec2,
}

/// Trazo en curso, en mm.
#[derive(Debug, Default)]
pub struct Pluma {
    pub nodos: Vec<Nodo>,
    /// Si se está arrastrando la manija del último nodo.
    pub arrastrando: bool,
}

impl Pluma {
    pub fn agregar(&mut self, p: Point) {
        self.nodos.push(Nodo { p, h: Vec2::ZERO });
    }

    /// Fija la manija del último nodo apuntando a `hacia`.
    pub fn manija(&mut self, hacia: Point) {
        if let Some(n) = self.nodos.last_mut() {
            n.h = hacia - n.p;
        }
    }

    /// Si se puede cerrar como zona.
    pub fn cerrable(&self) -> bool {
        self.nodos.len() >= 3
    }

    /// Trazado de los nodos; con `cerrar`, une el último con el primero y cierra.
    pub fn trazado(&self, cerrar: bool) -> BezPath {
        let mut path = BezPath::new();
        let Some(first) = self.nodos.first() else {
            return path;
        };
        path.move_to(first.p);
        for w in self.nodos.windows(2) {
            segmento(&mut path, w[0], w[1]);
        }
        if cerrar && self.cerrable() {
            segmento(&mut path, *self.nodos.last().unwrap(), *first);
            path.close_path();
        }
        path
    }
}

fn segmento(path: &mut BezPath, a: Nodo, b: Nodo) {
    if a.h == Vec2::ZERO && b.h == Vec2::ZERO {
        path.line_to(b.p);
    } else {
        path.curve_to(a.p + a.h, b.p - b.h, b.p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::PathEl;

    #[test]
    fn corners_are_lines_and_handles_are_curves() {
        let mut pen = Pluma::default();
        pen.agregar(Point::new(0.0, 0.0));
        pen.agregar(Point::new(10.0, 0.0));
        assert!(!pen.cerrable());
        pen.agregar(Point::new(10.0, 10.0));
        pen.manija(Point::new(10.0, 15.0));

        let open = pen.trazado(false);
        assert_eq!(open.elements()[1], PathEl::LineTo(Point::new(10.0, 0.0)));
        assert_eq!(
            open.elements()[2],
            PathEl::CurveTo(
                Point::new(10.0, 0.0),
                Point::new(10.0, 5.0),
                Point::new(10.0, 10.0)
            )
        );
        let closed = pen.trazado(true);
        assert_eq!(closed.elements().len(), 5);
        assert_eq!(closed.elements()[4], PathEl::ClosePath);
        // La curva de cierre sale con la manija del último nodo.
        assert!(
            matches!(closed.elements()[3], PathEl::CurveTo(a, _, _) if a == Point::new(10.0, 15.0))
        );
    }
}
