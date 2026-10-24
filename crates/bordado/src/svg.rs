//! Importar SVG: cada trazado se vuelve un objeto guía (sin puntada) con el color Janome más
//! parecido. Los textos se convierten a trazados con las fuentes del sistema.

use kurbo::{Affine, BezPath, Point, Shape};

use crate::proyecto::{Forma, OPACIDAD_GUIA, Objeto, Puntada};

/// Milímetros por unidad de usuario de SVG (96 por pulgada, como usan usvg y los navegadores).
const MM_POR_PX: f64 = 25.4 / 96.0;

#[derive(Debug, thiserror::Error)]
pub enum SvgError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("SVG inválido: {0}")]
    Svg(#[from] usvg::Error),
}

/// Lee un SVG y devuelve sus trazados en mm, centrados en el origen.
pub fn importar(data: &[u8]) -> Result<Vec<Objeto>, SvgError> {
    let mut opciones = usvg::Options::default();
    opciones.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_data(data, &opciones)?;
    let mut objetos = Vec::new();
    recorrer(tree.root(), &mut objetos);

    // Centrar el conjunto en el origen (el centro del bastidor).
    let caja = objetos
        .iter()
        .filter_map(|o| match &o.forma {
            Forma::Contorno { d } | Forma::Linea { d } => Some(d.bounding_box()),
            Forma::Texto { .. } => None,
        })
        .reduce(|a, b| a.union(b));
    if let Some(caja) = caja {
        let centro = caja.center();
        for o in &mut objetos {
            o.transformar(Affine::translate(Point::ZERO - centro));
        }
    }
    Ok(objetos)
}

pub fn importar_archivo(path: impl AsRef<std::path::Path>) -> Result<Vec<Objeto>, SvgError> {
    importar(&std::fs::read(path)?)
}

fn recorrer(grupo: &usvg::Group, out: &mut Vec<Objeto>) {
    for nodo in grupo.children() {
        match nodo {
            usvg::Node::Group(g) => recorrer(g, out),
            usvg::Node::Text(t) => recorrer(t.flattened(), out),
            usvg::Node::Path(p) if p.is_visible() => {
                if let Some(o) = objeto(p, out.len() + 1) {
                    out.push(o);
                }
            }
            usvg::Node::Path(_) | usvg::Node::Image(_) => {}
        }
    }
}

fn objeto(p: &usvg::Path, n: usize) -> Option<Objeto> {
    let t = p.abs_transform();
    let afin =
        Affine::scale(MM_POR_PX) * Affine::new([t.sx, t.ky, t.kx, t.sy, t.tx, t.ty].map(f64::from));
    let mut d = BezPath::new();
    let mut cerrado = true;
    let mut abierto = false;
    let pt = |q: usvg::tiny_skia_path::Point| afin * Point::new(f64::from(q.x), f64::from(q.y));
    for seg in p.data().segments() {
        use usvg::tiny_skia_path::PathSegment as S;
        match seg {
            S::MoveTo(a) => {
                if abierto {
                    cerrado = false;
                }
                abierto = true;
                d.move_to(pt(a));
            }
            S::LineTo(a) => d.line_to(pt(a)),
            S::QuadTo(a, b) => d.quad_to(pt(a), pt(b)),
            S::CubicTo(a, b, c) => d.curve_to(pt(a), pt(b), pt(c)),
            S::Close => {
                abierto = false;
                d.close_path();
            }
        }
    }
    if abierto {
        cerrado = false;
    }
    if d.elements().len() < 2 {
        return None;
    }

    let pintura = p
        .fill()
        .map(|f| f.paint())
        .or_else(|| p.stroke().map(|s| s.paint()));
    let color = match pintura {
        Some(usvg::Paint::Color(c)) => jef::palette::nearest([c.red, c.green, c.blue]),
        _ => 1,
    };
    // Un trazado con relleno es una zona aunque no termine en Z (SVG lo cierra solo).
    let forma = if cerrado || p.fill().is_some() {
        Forma::Contorno { d }
    } else {
        Forma::Linea { d }
    };
    let nombre = if p.id().is_empty() {
        format!("trazado {n}")
    } else {
        p.id().to_string()
    };
    Some(Objeto {
        nombre,
        visible: true,
        color,
        forma,
        puntada: Puntada::Ninguna,
        opacidad: OPACIDAD_GUIA,
        bloqueado: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm"
        viewBox="0 0 200 100">
      <rect id="caja" x="0" y="0" width="200" height="100" fill="#ff0000"/>
      <g transform="translate(100 50)">
        <circle cx="0" cy="0" r="20" fill="none" stroke="#000"/>
        <path d="M-50,0 L50,0" stroke="#0000ff" fill="none"/>
      </g>
    </svg>"##;

    #[test]
    fn imports_shapes_in_mm_centered() {
        let objs = importar(SVG.as_bytes()).unwrap();
        assert_eq!(objs.len(), 3);
        let Forma::Contorno { d } = &objs[0].forma else {
            panic!("el rectángulo es un contorno");
        };
        // viewBox de 200 en 100 mm: escala 0,5 mm por unidad.
        let b = d.bounding_box();
        assert!((b.width() - 100.0).abs() < 1e-3 && (b.height() - 50.0).abs() < 1e-3);
        assert!(b.center().distance(Point::ZERO) < 1e-3);
        assert_eq!(objs[0].nombre, "caja");
        assert_eq!(objs[0].color, 10, "rojo");
        assert_eq!(objs[0].puntada, Puntada::Ninguna);

        let Forma::Contorno { d } = &objs[1].forma else {
            panic!("el círculo es cerrado");
        };
        assert!((d.bounding_box().width() - 20.0).abs() < 1e-3);
        assert!(d.bounding_box().center().distance(Point::ZERO) < 1e-3);
        assert!(matches!(objs[2].forma, Forma::Linea { .. }));
    }
}
