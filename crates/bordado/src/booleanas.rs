//! Operaciones booleanas entre zonas (unir, restar, intersecar, excluir).
//!
//! Las curvas se aproximan con segmentos (`geom::TOLERANCIA`) y se usa la regla par-impar,
//! igual que el relleno. El resultado es un trazado de solo rectas.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use kurbo::{BezPath, Point};

use crate::geom::polilineas;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Booleana {
    Unir,
    /// El primero menos los demás.
    Restar,
    Intersecar,
    /// Lo que cubre uno solo de los dos.
    Excluir,
}

impl Booleana {
    pub const TODAS: [Booleana; 4] = [
        Booleana::Unir,
        Booleana::Restar,
        Booleana::Intersecar,
        Booleana::Excluir,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Booleana::Unir => "Unir",
            Booleana::Restar => "Restar",
            Booleana::Intersecar => "Intersecar",
            Booleana::Excluir => "Excluir",
        }
    }
}

/// Aplica la operación a dos zonas. Todos los subtrazados cuentan como cerrados (una zona
/// de SVG con relleno no siempre termina en `Z`).
pub fn operar(op: Booleana, a: &BezPath, b: &BezPath) -> BezPath {
    let regla = match op {
        Booleana::Unir => OverlayRule::Union,
        Booleana::Restar => OverlayRule::Difference,
        Booleana::Intersecar => OverlayRule::Intersect,
        Booleana::Excluir => OverlayRule::Xor,
    };
    let formas = contornos(a).overlay(&contornos(b), regla, FillRule::EvenOdd);
    let mut d = BezPath::new();
    for contorno in formas.into_iter().flatten() {
        let mut puntos = contorno.into_iter().map(|[x, y]| Point::new(x, y));
        let Some(p) = puntos.next() else { continue };
        d.move_to(p);
        for p in puntos {
            d.line_to(p);
        }
        d.close_path();
    }
    d
}

/// Aplica la operación de a pares: `((a op b) op c) …`. `None` si no hay zonas.
pub fn operar_varias<'a>(
    op: Booleana,
    zonas: impl IntoIterator<Item = &'a BezPath>,
) -> Option<BezPath> {
    let mut zonas = zonas.into_iter();
    let primera = zonas.next()?.clone();
    Some(zonas.fold(primera, |acc, z| operar(op, &acc, z)))
}

fn contornos(d: &BezPath) -> Vec<Vec<[f64; 2]>> {
    polilineas(d)
        .into_iter()
        .filter(|pl| pl.puntos.len() >= 3)
        .map(|pl| pl.puntos.iter().map(|p| [p.x, p.y]).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn cuadrado(x: f64, y: f64, lado: f64) -> BezPath {
        BezPath::from_svg(&format!("M{x},{y} h{lado} v{lado} h-{lado} Z")).unwrap()
    }

    #[test]
    fn two_overlapping_squares() {
        let a = cuadrado(0.0, 0.0, 10.0);
        let b = cuadrado(5.0, 5.0, 10.0);
        for (op, area) in [
            (Booleana::Unir, 175.0),
            (Booleana::Restar, 75.0),
            (Booleana::Intersecar, 25.0),
            (Booleana::Excluir, 150.0),
        ] {
            let r = operar(op, &a, &b);
            assert!((r.area().abs() - area).abs() < 1e-6, "{op:?}: {}", r.area());
        }
    }

    #[test]
    fn subtracting_inner_square_leaves_hole() {
        let r = operar(
            Booleana::Restar,
            &cuadrado(0.0, 0.0, 10.0),
            &cuadrado(4.0, 4.0, 2.0),
        );
        assert_eq!(polilineas(&r).len(), 2, "borde y agujero");
        assert!((r.area().abs() - 96.0).abs() < 1e-6);
    }

    #[test]
    fn holes_follow_even_odd() {
        // Un anillo (cuadrado con agujero) unido a algo que tapa el agujero queda lleno.
        let mut anillo = cuadrado(0.0, 0.0, 10.0);
        anillo.extend(cuadrado(3.0, 3.0, 4.0));
        let r = operar(Booleana::Unir, &anillo, &cuadrado(2.0, 2.0, 6.0));
        assert!((r.area().abs() - 100.0).abs() < 1e-6);
        assert!(operar_varias(Booleana::Unir, []).is_none());
    }

    #[test]
    fn zone_without_z_counts_as_closed() {
        let a = BezPath::from_svg("M0,0 L10,0 L10,10 L0,10").unwrap();
        let r = operar(Booleana::Unir, &a, &cuadrado(5.0, 5.0, 10.0));
        assert!((r.area().abs() - 175.0).abs() < 1e-6);
    }

    #[test]
    fn disjoint_intersection_is_empty() {
        let r = operar(
            Booleana::Intersecar,
            &cuadrado(0.0, 0.0, 1.0),
            &cuadrado(5.0, 5.0, 1.0),
        );
        assert!(r.elements().is_empty());
    }
}
