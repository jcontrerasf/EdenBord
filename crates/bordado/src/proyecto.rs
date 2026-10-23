//! Modelo del proyecto editable `.bordado` (JSON).
//!
//! Todo va en milímetros, con y hacia abajo y el origen en el centro del bastidor. Las formas
//! se guardan como el atributo `d` de SVG para que el archivo sea legible y fácil de editar.

use std::path::{Path, PathBuf};

use kurbo::{Affine, BezPath, PathEl};
use serde::{Deserialize, Serialize};

/// Valor del campo `formato`, para reconocer el archivo.
pub const FORMATO: &str = "bordado";
/// Versión del formato que escribe esta librería.
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proyecto {
    pub formato: String,
    pub version: u32,
    /// Código de bastidor JEF (ver [`jef::Hoop::code`]).
    pub bastidor: i32,
    /// Imagen para calcar; no se borda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fondo: Option<Fondo>,
    /// Objetos en orden de bordado.
    pub objetos: Vec<Objeto>,
}

/// Imagen de referencia bajo el diseño.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fondo {
    /// Ruta de la imagen; si es relativa, se toma desde la carpeta del proyecto.
    pub ruta: PathBuf,
    /// Centro de la imagen, en mm.
    pub centro: [f64; 2],
    /// Ancho en mm; el alto sale de la proporción de la imagen.
    pub ancho: f64,
    /// 0 = invisible, 1 = opaca.
    pub opacidad: f32,
    #[serde(default = "si")]
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objeto {
    pub nombre: String,
    #[serde(default = "si")]
    pub visible: bool,
    /// Índice en la paleta Janome.
    pub color: i32,
    pub forma: Forma,
    pub puntada: Puntada,
}

fn si() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Forma {
    /// Zona cerrada; los subtrazados interiores son agujeros (regla par-impar).
    Contorno {
        #[serde(with = "svg_d")]
        d: BezPath,
    },
    /// Trazado abierto (o varios).
    Linea {
        #[serde(with = "svg_d")]
        d: BezPath,
    },
    /// Texto con una fuente TrueType/OpenType; se convierte en contorno al generar.
    Texto {
        texto: String,
        fuente: PathBuf,
        /// Alto de las mayúsculas, en mm.
        alto: f64,
        /// Espacio extra entre letras, en mm.
        #[serde(default)]
        espaciado: f64,
        /// Posición, giro y escala: lleva el texto (línea base en y = 0, desde x = 0) a su lugar.
        afin: [f64; 6],
    },
}

/// Tipo de puntada y sus parámetros (mm y grados).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Puntada {
    /// Guía: se ve en el editor pero no se borda.
    Ninguna,
    /// Pespunte sobre el trazado (en los contornos, sobre el borde).
    Corrido {
        largo: f64,
        /// Cada tramo ida, vuelta e ida: línea más gruesa.
        triple: bool,
    },
    /// Columna en zigzag a lo largo del trazado.
    Satin {
        ancho: f64,
        /// Distancia entre dos puntadas seguidas del mismo lado.
        densidad: f64,
        /// Pespunte por el centro antes del satín, para asentarlo.
        base: bool,
    },
    /// Relleno tatami de una zona cerrada.
    Relleno {
        /// Dirección de las filas; 0 = horizontal, positivo = horario en pantalla.
        angulo: f64,
        /// Distancia entre filas.
        separacion: f64,
        largo: f64,
        /// Relleno ralo cruzado antes del relleno, para que no se hunda en la tela.
        base: bool,
        /// Cuánto se alarga cada fila en sus extremos para compensar el tirón del hilo.
        compensacion: f64,
    },
}

impl Puntada {
    pub fn nombre(&self) -> &'static str {
        match self {
            Puntada::Ninguna => "Guía (no se borda)",
            Puntada::Corrido { .. } => "Pespunte",
            Puntada::Satin { .. } => "Satín",
            Puntada::Relleno { .. } => "Relleno",
        }
    }
}

/// Tipos de puntada listos para usar; los parámetros se ajustan después.
pub const PRESETS: &[(&str, Puntada)] = &[
    ("Guía (no se borda)", Puntada::Ninguna),
    (
        "Pespunte",
        Puntada::Corrido {
            largo: 2.5,
            triple: false,
        },
    ),
    (
        "Pespunte triple",
        Puntada::Corrido {
            largo: 2.5,
            triple: true,
        },
    ),
    (
        "Satín 2 mm",
        Puntada::Satin {
            ancho: 2.0,
            densidad: 0.4,
            base: true,
        },
    ),
    (
        "Satín 4 mm",
        Puntada::Satin {
            ancho: 4.0,
            densidad: 0.4,
            base: true,
        },
    ),
    (
        "Relleno 45°",
        Puntada::Relleno {
            angulo: 45.0,
            separacion: 0.4,
            largo: 3.5,
            base: true,
            compensacion: 0.2,
        },
    ),
    (
        "Relleno horizontal",
        Puntada::Relleno {
            angulo: 0.0,
            separacion: 0.4,
            largo: 3.5,
            base: true,
            compensacion: 0.2,
        },
    ),
    (
        "Relleno liviano",
        Puntada::Relleno {
            angulo: 45.0,
            separacion: 0.7,
            largo: 4.0,
            base: false,
            compensacion: 0.1,
        },
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum ProyectoError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no es un proyecto de bordado (formato «{0}»)")]
    Formato(String),
    #[error("versión {0} no soportada (esta versión lee hasta la {VERSION})")]
    Version(u32),
}

impl Default for Proyecto {
    fn default() -> Self {
        Proyecto {
            formato: FORMATO.into(),
            version: VERSION,
            bastidor: jef::Hoop::A110x110.code(),
            fondo: None,
            objetos: vec![],
        }
    }
}

impl Proyecto {
    pub fn leer(texto: &str) -> Result<Self, ProyectoError> {
        let p: Proyecto = serde_json::from_str(texto)?;
        if p.formato != FORMATO {
            return Err(ProyectoError::Formato(p.formato));
        }
        if p.version > VERSION {
            return Err(ProyectoError::Version(p.version));
        }
        Ok(p)
    }

    pub fn guardar(&self) -> String {
        // Solo falla con mapas de claves no textuales, que el modelo no tiene.
        serde_json::to_string_pretty(self).expect("el proyecto siempre se puede serializar")
    }

    pub fn leer_archivo(path: impl AsRef<Path>) -> Result<Self, ProyectoError> {
        Self::leer(&std::fs::read_to_string(path)?)
    }
}

impl Objeto {
    /// Aplica una transformación (en mm) a la forma.
    pub fn transformar(&mut self, t: Affine) {
        match &mut self.forma {
            Forma::Contorno { d } | Forma::Linea { d } => d.apply_affine(t),
            Forma::Texto { afin, .. } => *afin = (t * Affine::new(*afin)).as_coeffs(),
        }
    }
}

/// (De)serializa un [`BezPath`] como texto `d` de SVG, con coordenadas redondeadas a µm.
mod svg_d {
    use std::fmt::Write;

    use super::*;

    pub fn serialize<S: serde::Serializer>(path: &BezPath, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&to_svg(path))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BezPath, D::Error> {
        let text = String::deserialize(d)?;
        BezPath::from_svg(&text).map_err(serde::de::Error::custom)
    }

    fn to_svg(path: &BezPath) -> String {
        let mut out = String::new();
        let mut put = |cmd: &str, pts: &[kurbo::Point]| {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(cmd);
            for (i, p) in pts.iter().enumerate() {
                let sep = if i == 0 { "" } else { " " };
                let _ = write!(out, "{sep}{},{}", num(p.x), num(p.y));
            }
        };
        for el in path.elements() {
            match *el {
                PathEl::MoveTo(p) => put("M", &[p]),
                PathEl::LineTo(p) => put("L", &[p]),
                PathEl::QuadTo(a, b) => put("Q", &[a, b]),
                PathEl::CurveTo(a, b, c) => put("C", &[a, b, c]),
                PathEl::ClosePath => put("Z", &[]),
            }
        }
        out
    }

    fn num(v: f64) -> String {
        let r = (v * 1000.0).round() / 1000.0;
        // Evita "-0".
        format!("{}", if r == 0.0 { 0.0 } else { r })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip() {
        let mut p = Proyecto::default();
        p.objetos.push(Objeto {
            nombre: "cuadrado".into(),
            visible: true,
            color: 10,
            forma: Forma::Contorno {
                d: BezPath::from_svg("M0,0 L10.5,0 L10.5,-3.25 Q5,2 0,0 Z").unwrap(),
            },
            puntada: PRESETS[5].1.clone(),
        });
        p.objetos.push(Objeto {
            nombre: "texto".into(),
            visible: false,
            color: 1,
            forma: Forma::Texto {
                texto: "Hola".into(),
                fuente: "fuente.ttf".into(),
                alto: 8.0,
                espaciado: 0.5,
                afin: Affine::translate((1.0, 2.0)).as_coeffs(),
            },
            puntada: Puntada::Ninguna,
        });
        let text = p.guardar();
        assert!(!text.contains("fondo"), "sin fondo no se escribe el campo");
        p.fondo = Some(Fondo {
            ruta: "foto.png".into(),
            centro: [1.5, -2.0],
            ancho: 80.0,
            opacidad: 0.5,
            visible: true,
        });
        let text = p.guardar();
        assert!(text.contains("\"d\": \"M0,0 L10.5,0 L10.5,-3.25 Q5,2 0,0 Z\""));
        let back = Proyecto::leer(&text).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.guardar(), text);
    }

    #[test]
    fn rejects_other_formats() {
        let text = r#"{"formato":"otro","version":1,"bastidor":0,"objetos":[]}"#;
        assert!(matches!(
            Proyecto::leer(text),
            Err(ProyectoError::Formato(_))
        ));
        let text = r#"{"formato":"bordado","version":99,"bastidor":0,"objetos":[]}"#;
        assert!(matches!(
            Proyecto::leer(text),
            Err(ProyectoError::Version(99))
        ));
    }
}
