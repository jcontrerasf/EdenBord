//! Proyectos de bordado editables (`.edenbord`) y su conversión a puntadas JEF.
//!
//! Un proyecto es una lista de objetos (formas en mm con un color y un tipo de puntada).
//! [`Proyecto::generar`] calcula las puntadas; el proyecto se puede seguir editando y volver
//! a generar, cosa que un JEF no permite.

pub mod geom;
mod proyecto;
pub mod puntadas;
pub mod svg;
pub mod texto;

use kurbo::{Affine, BezPath, Point};

pub use proyecto::{
    EXTENSION, FORMATO, FORMATO_ANTIGUO, Fondo, Forma, Objeto, PRESETS, Proyecto, ProyectoError,
    Puntada, VERSION,
};

use geom::{Polilinea, polilineas};
use jef::{Hoop, Jef, Stitch, StitchKind};
use puntadas::Tramo;

/// Parámetros de la base del relleno: ralo, cruzado y metido hacia adentro.
const BASE_SEPARACION: f64 = 2.0;
const BASE_LARGO: f64 = 3.0;
const BASE_MARGEN: f64 = -0.5;

impl Objeto {
    /// Si la forma es una zona (se puede rellenar).
    pub fn es_zona(&self) -> bool {
        !matches!(self.forma, Forma::Linea { .. })
    }

    /// La forma como trazado en mm.
    pub fn trazado(&self) -> Result<BezPath, String> {
        match &self.forma {
            Forma::Contorno { d } | Forma::Linea { d } => Ok(d.clone()),
            Forma::Texto {
                texto,
                fuente,
                alto,
                espaciado,
                afin,
            } => {
                let mut d = texto::trazado(texto, fuente, *alto, *espaciado)?;
                d.apply_affine(Affine::new(*afin));
                Ok(d)
            }
        }
    }

    /// Tramos de puntadas en mm. `desde` es donde está la aguja.
    pub fn tramos(&self, desde: Point) -> Result<Vec<Tramo>, String> {
        let lineas = polilineas(&self.trazado()?);
        Ok(match self.puntada {
            Puntada::Ninguna => vec![],
            Puntada::Corrido { largo, triple } => puntadas::corrido(&lineas, largo, triple),
            Puntada::Satin {
                ancho,
                densidad,
                base,
            } => puntadas::satin(&lineas, ancho, densidad, base),
            Puntada::Relleno {
                angulo,
                separacion,
                largo,
                base,
                compensacion,
            } => {
                // Como en SVG, un trazado abierto se rellena como si estuviera cerrado.
                let anillos: Vec<_> = lineas
                    .into_iter()
                    .map(|Polilinea { puntos, .. }| puntos)
                    .collect();
                let mut out = Vec::new();
                let mut desde = desde;
                if base {
                    out = puntadas::relleno(
                        &anillos,
                        angulo + 90.0,
                        BASE_SEPARACION,
                        BASE_LARGO,
                        BASE_MARGEN,
                        desde,
                    );
                    desde = out.last().and_then(|t| t.last().copied()).unwrap_or(desde);
                }
                out.extend(puntadas::relleno(
                    &anillos,
                    angulo,
                    separacion,
                    largo,
                    compensacion,
                    desde,
                ));
                out
            }
        })
    }
}

/// Resultado de generar: el diseño y los avisos de objetos que no se pudieron bordar.
pub struct Generado {
    pub jef: Jef,
    /// Para cada bloque de color del JEF, el índice del primer objeto que lo forma.
    pub objeto_de_bloque: Vec<usize>,
    pub avisos: Vec<String>,
}

impl Proyecto {
    /// Calcula las puntadas de los objetos visibles. Los tramos se unen con saltos y remates;
    /// objetos seguidos del mismo color no cambian de hilo.
    pub fn generar(&self) -> Generado {
        let mut stitches: Vec<Stitch> = Vec::new();
        let mut colors = Vec::new();
        let mut objeto_de_bloque = Vec::new();
        let mut avisos = Vec::new();
        let mut aguja = Point::ZERO;
        let mut pos = (0, 0);
        for (i, obj) in self.objetos.iter().enumerate() {
            if !obj.visible {
                continue;
            }
            let tramos = match obj.tramos(aguja) {
                Ok(t) => t,
                Err(e) => {
                    avisos.push(format!("{}: {e}", obj.nombre));
                    continue;
                }
            };
            let tramos: Vec<Vec<(i32, i32)>> = tramos
                .iter()
                .map(|t| {
                    let mut q: Vec<_> = puntadas::con_remates(t).into_iter().map(decimas).collect();
                    q.dedup();
                    q
                })
                .filter(|t| t.len() > 1)
                .collect();
            if tramos.is_empty() {
                continue;
            }
            if colors.last() != Some(&obj.color) {
                if !colors.is_empty() {
                    stitches.push(stitch(pos, StitchKind::ColorChange));
                }
                colors.push(obj.color);
                objeto_de_bloque.push(i);
            }
            for t in tramos {
                if t[0] != pos {
                    stitches.push(stitch(t[0], StitchKind::Jump));
                }
                stitches.extend(t[1..].iter().map(|&p| stitch(p, StitchKind::Normal)));
                pos = *t.last().unwrap();
            }
            aguja = Point::new(f64::from(pos.0) / 10.0, f64::from(pos.1) / 10.0);
        }
        if colors.is_empty() {
            colors.push(self.objetos.first().map_or(1, |o| o.color));
        }
        Generado {
            jef: Jef::new(Hoop::from_code(self.bastidor), colors, stitches),
            objeto_de_bloque,
            avisos,
        }
    }
}

/// Punto en mm a 0,1 mm enteros.
fn decimas(p: Point) -> (i32, i32) {
    ((p.x * 10.0).round() as i32, (p.y * 10.0).round() as i32)
}

fn stitch((x, y): (i32, i32), kind: StitchKind) -> Stitch {
    Stitch { x, y, kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objeto(d: &str, color: i32, puntada: Puntada) -> Objeto {
        Objeto {
            nombre: "x".into(),
            visible: true,
            color,
            forma: Forma::Contorno {
                d: BezPath::from_svg(d).unwrap(),
            },
            puntada,
        }
    }

    #[test]
    fn generated_design_roundtrips_and_changes_color_only_when_needed() {
        let mut p = Proyecto::default();
        let cuadrado = "M-20,-20 L20,-20 L20,20 L-20,20 Z M-5,-5 L5,-5 L5,5 L-5,5 Z";
        p.objetos.push(objeto(cuadrado, 10, PRESETS[5].1.clone()));
        p.objetos.push(objeto(cuadrado, 10, PRESETS[3].1.clone()));
        p.objetos
            .push(objeto("M30,0 L40,0", 1, PRESETS[2].1.clone()));
        p.objetos.push(objeto("M0,0 L1,1", 5, Puntada::Ninguna));
        let g = p.generar();
        assert!(g.avisos.is_empty());
        assert_eq!(g.jef.colors, vec![10, 1]);
        assert_eq!(g.objeto_de_bloque, vec![0, 2]);
        assert_eq!(g.jef.count(StitchKind::ColorChange), 1);
        assert!(g.jef.count(StitchKind::Normal) > 1000);

        // Ninguna puntada dentro del agujero, salvo la compensación y medio ancho del satín.
        for s in &g.jef.stitches {
            if s.kind == StitchKind::Normal {
                assert!(!(s.x.abs() < 38 && s.y.abs() < 38), "{s:?}");
            }
        }

        let bytes = jef::write(&g.jef);
        let back = jef::read(&bytes).unwrap();
        assert_eq!(jef::write(&back), bytes);
    }

    #[test]
    fn hidden_guides_and_errors_produce_valid_empty_design() {
        let mut p = Proyecto::default();
        p.objetos.push(Objeto {
            nombre: "texto".into(),
            visible: true,
            color: 3,
            forma: Forma::Texto {
                texto: "A".into(),
                fuente: "no-existe.ttf".into(),
                alto: 10.0,
                espaciado: 0.0,
                afin: Affine::IDENTITY.as_coeffs(),
            },
            puntada: PRESETS[5].1.clone(),
        });
        let g = p.generar();
        assert_eq!(g.avisos.len(), 1);
        assert_eq!(g.jef.colors, vec![3]);
        assert!(g.jef.stitches.is_empty());
        assert!(jef::read(&jef::write(&g.jef)).is_ok());
    }
}
