//! Preferencias que se recuerdan entre sesiones (eframe las guarda con su almacenamiento).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Clave en el almacenamiento de eframe.
pub const CLAVE: &str = "preferencias";
/// Cuántos archivos recientes se recuerdan.
const MAX_RECIENTES: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferencias {
    /// Archivos abiertos o guardados, del más nuevo al más viejo.
    pub recientes: Vec<PathBuf>,
    /// Zoom de la interfaz (Ctrl + / Ctrl − / Ctrl 0).
    pub zoom_ui: f32,
    pub saltos: bool,
    pub bastidor: bool,
    pub cuadricula: bool,
    pub contornos: bool,
    /// Vista realista de las puntadas.
    pub realista: bool,
    /// Fuente de los textos nuevos (la última elegida).
    pub fuente: Option<bordado::fuentes::Fuente>,
    /// Color de la tela (RGB).
    pub tela: [u8; 3],
}

impl Default for Preferencias {
    fn default() -> Self {
        Self {
            recientes: vec![],
            zoom_ui: 1.0,
            saltos: false,
            bastidor: true,
            cuadricula: true,
            contornos: false,
            realista: true,
            fuente: None,
            tela: [196, 190, 178],
        }
    }
}

/// Pone `path` primero en la lista de recientes, sin repetir y con un máximo.
pub fn agregar_reciente(recientes: &mut Vec<PathBuf>, path: &Path) {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    recientes.retain(|p| *p != path);
    recientes.insert(0, path);
    recientes.truncate(MAX_RECIENTES);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recientes_sin_repetir_y_con_maximo() {
        let mut r = vec![];
        for i in 0..12 {
            agregar_reciente(&mut r, Path::new(&format!("no-existe-{i}.jef")));
        }
        assert_eq!(r.len(), MAX_RECIENTES);
        assert_eq!(r[0], PathBuf::from("no-existe-11.jef"));
        agregar_reciente(&mut r, Path::new("no-existe-5.jef"));
        assert_eq!(r[0], PathBuf::from("no-existe-5.jef"));
        assert_eq!(
            r.iter().filter(|p| p.ends_with("no-existe-5.jef")).count(),
            1
        );
        assert_eq!(r.len(), MAX_RECIENTES);
    }

    #[test]
    fn campos_faltantes_toman_el_valor_por_defecto() {
        let p: Preferencias = serde_json::from_str(r#"{"saltos":true}"#).unwrap();
        assert!(p.saltos);
        assert_eq!(p.zoom_ui, 1.0);
        assert!(p.bastidor);
        assert!(
            p.realista,
            "las preferencias viejas abren con la vista realista"
        );
    }
}
