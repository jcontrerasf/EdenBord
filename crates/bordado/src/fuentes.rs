//! Fuentes de los textos: las instaladas en el sistema (por familia y estilo) o un archivo
//! `.ttf`/`.otf` propio.
//!
//! Un texto guarda la familia y el estilo, que sirven en cualquier computador que tenga la
//! fuente, y la ruta del archivo como respaldo (o como única referencia, si es una fuente
//! propia). La base de fuentes del sistema se lee una vez y la comparte la importación de SVG.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use usvg::fontdb::{self, Database, Family, Query, Source, Stretch, Style, Weight};

/// Peso normal (CSS / OpenType).
pub const NORMAL: u16 = 400;
pub const NEGRITA: u16 = 700;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fuente {
    /// Familia instalada en el sistema («DejaVu Sans»). `None` = solo el archivo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub familia: Option<String>,
    /// 100 (fina) a 900 (gruesa); 400 normal, 700 negrita.
    #[serde(default = "normal", skip_serializing_if = "es_normal")]
    pub peso: u16,
    #[serde(default, skip_serializing_if = "no")]
    pub cursiva: bool,
    /// Archivo de la fuente: la propia, o de respaldo si la familia no está instalada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruta: Option<PathBuf>,
}

fn normal() -> u16 {
    NORMAL
}

fn es_normal(p: &u16) -> bool {
    *p == NORMAL
}

fn no(b: &bool) -> bool {
    !*b
}

impl Fuente {
    /// Una fuente propia, de un archivo.
    pub fn archivo(ruta: impl Into<PathBuf>) -> Self {
        Fuente {
            familia: None,
            peso: NORMAL,
            cursiva: false,
            ruta: Some(ruta.into()),
        }
    }

    /// Una familia del sistema con su estilo; guarda como respaldo la ruta del archivo que le
    /// toca en este computador.
    pub fn sistema(familia: &str, peso: u16, cursiva: bool) -> Self {
        let mut f = Fuente {
            familia: Some(familia.to_string()),
            peso,
            cursiva,
            ruta: None,
        };
        let db = base();
        if let Some(id) = buscar(db, &f)
            && let Some((Source::File(ruta), _)) = db.face_source(id)
        {
            f.ruta = Some(ruta);
        }
        f
    }

    /// Nombre para mostrar: «DejaVu Sans Negrita cursiva» o el nombre del archivo.
    pub fn nombre(&self) -> String {
        match &self.familia {
            Some(familia) => match estilo(self.peso, self.cursiva) {
                e if e == "Normal" => familia.clone(),
                e => format!("{familia} {e}"),
            },
            None => self
                .ruta
                .as_deref()
                .and_then(Path::file_name)
                .map_or("(sin fuente)".into(), |n| n.to_string_lossy().into_owned()),
        }
    }
}

/// Nombre de un estilo: «Negrita», «Ligera cursiva»…
pub fn estilo(peso: u16, cursiva: bool) -> String {
    let p = match peso {
        0..=149 => "Fina",
        150..=249 => "Extraligera",
        250..=349 => "Ligera",
        350..=449 => "Normal",
        450..=549 => "Media",
        550..=649 => "Seminegrita",
        650..=749 => "Negrita",
        750..=849 => "Extranegrita",
        _ => "Gruesa",
    };
    match (p, cursiva) {
        ("Normal", true) => "Cursiva".into(),
        (p, true) => format!("{p} cursiva"),
        (p, false) => p.into(),
    }
}

/// Las fuentes del sistema, leídas la primera vez que hacen falta.
pub fn base() -> &'static Arc<Database> {
    static BASE: OnceLock<Arc<Database>> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut db = Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
}

/// Familias instaladas, ordenadas y sin repetir.
pub fn familias() -> &'static [String] {
    static FAMILIAS: OnceLock<Vec<String>> = OnceLock::new();
    FAMILIAS.get_or_init(|| {
        let mut v: Vec<String> = base()
            .faces()
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        v.sort_by_key(|n| n.to_lowercase());
        v.dedup();
        v
    })
}

/// Estilos que tiene una familia: (peso, cursiva), ordenados.
pub fn estilos(familia: &str) -> Vec<(u16, bool)> {
    let mut v: Vec<(u16, bool)> = base()
        .faces()
        .filter(|f| f.families.iter().any(|(n, _)| n == familia))
        .map(|f| (f.weight.0, f.style != Style::Normal))
        .collect();
    v.sort_by_key(|&(p, c)| (c, p));
    v.dedup();
    v
}

/// Sans serif conocidas, en orden de preferencia, para los textos nuevos (fontdb no lee la
/// predeterminada de fontconfig: su «sans-serif» suele ser Arial o FreeSans).
const PREFERIDAS: [&str; 8] = [
    "Noto Sans",
    "Segoe UI",
    "DejaVu Sans",
    "Cantarell",
    "Ubuntu",
    "Arial",
    "Helvetica",
    "Liberation Sans",
];

/// La fuente para textos nuevos: una sans serif común si está instalada; si no, la sans serif
/// de fontdb o la primera que haya.
pub fn predeterminada() -> Option<Fuente> {
    let hay = |f: &str| familias().iter().any(|g| g == f);
    let sans = base().family_name(&Family::SansSerif).to_string();
    let familia = PREFERIDAS
        .iter()
        .map(|f| f.to_string())
        .chain([sans])
        .find(|f| hay(f))
        .or_else(|| familias().first().cloned())?;
    Some(Fuente::sistema(&familia, NORMAL, false))
}

fn buscar(db: &Database, f: &Fuente) -> Option<fontdb::ID> {
    let familia = f.familia.as_deref()?;
    // `query` devuelve la cara más parecida de la familia; si la familia no está, nada.
    db.query(&Query {
        families: &[Family::Name(familia)],
        weight: Weight(f.peso),
        stretch: Stretch::Normal,
        style: if f.cursiva {
            Style::Italic
        } else {
            Style::Normal
        },
    })
}

/// Datos de la fuente ya leídos, para no buscarlos cada vez que se regenera el diseño.
type Datos = (Arc<Vec<u8>>, u32);
/// Fuentes en la caché como máximo (la vista previa del selector va cargando muchas, y algunas
/// pesan decenas de MB).
const MAX_CACHE: usize = 12;
static CACHE: Mutex<Option<HashMap<Fuente, Datos>>> = Mutex::new(None);

/// Bytes del archivo de la fuente y el índice de la cara (en colecciones `.ttc`). Busca la
/// familia en el sistema; si no está, usa la ruta.
pub fn datos(f: &Fuente) -> Result<Datos, String> {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_default();
    if let Some(d) = cache.get(f) {
        return Ok(d.clone());
    }
    let db = base();
    let d = match buscar(db, f) {
        Some(id) => db
            .with_face_data(id, |data, index| (Arc::new(data.to_vec()), index))
            .ok_or_else(|| format!("no se pudo leer la fuente «{}»", f.nombre()))?,
        None => {
            let Some(ruta) = &f.ruta else {
                return Err(format!("no está instalada la fuente «{}»", f.nombre()));
            };
            let data = std::fs::read(ruta).map_err(|e| match &f.familia {
                Some(familia) => format!("no está instalada la fuente «{familia}»: {e}"),
                None => format!("no se pudo leer la fuente {}: {e}", ruta.display()),
            })?;
            (Arc::new(data), 0)
        }
    };
    if cache.len() >= MAX_CACHE {
        cache.clear();
    }
    cache.insert(f.clone(), d.clone());
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_short_for_the_usual_case() {
        let f = Fuente {
            familia: Some("DejaVu Sans".into()),
            peso: NORMAL,
            cursiva: false,
            ruta: None,
        };
        assert_eq!(
            serde_json::to_string(&f).unwrap(),
            r#"{"familia":"DejaVu Sans"}"#
        );
        let f: Fuente = serde_json::from_str(r#"{"ruta":"a.ttf","peso":700}"#).unwrap();
        assert_eq!(f.peso, NEGRITA);
        assert_eq!(f.nombre(), "a.ttf");
    }

    #[test]
    fn style_names() {
        assert_eq!(estilo(400, false), "Normal");
        assert_eq!(estilo(400, true), "Cursiva");
        assert_eq!(estilo(700, true), "Negrita cursiva");
        let mut f = Fuente::archivo("x.ttf");
        f.familia = Some("Inter".into());
        f.peso = 300;
        assert_eq!(f.nombre(), "Inter Ligera");
    }

    #[test]
    fn missing_family_falls_back_to_the_file_or_fails() {
        let mut f = Fuente::archivo("no-existe.ttf");
        f.familia = Some("Familia Que No Existe 123".into());
        let e = datos(&f).unwrap_err();
        assert!(e.contains("no está instalada"), "{e}");
    }

    #[test]
    fn system_default_resolves() {
        let Some(f) = predeterminada() else {
            eprintln!("aviso: el sistema no tiene fuentes; prueba saltada");
            return;
        };
        let (data, index) = datos(&f).unwrap();
        assert!(ttf_parser::Face::parse(&data, index).is_ok());
        assert!(!estilos(f.familia.as_deref().unwrap()).is_empty());
    }
}
