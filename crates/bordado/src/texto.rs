//! Texto a trazados con una fuente TrueType/OpenType (sin kerning ni ligaduras).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kurbo::{BezPath, Point};

/// Fuentes ya leídas, para no ir a disco cada vez que se regenera el diseño.
static FUENTES: Mutex<Option<HashMap<PathBuf, Arc<Vec<u8>>>>> = Mutex::new(None);

fn fuente(path: &Path) -> Result<Arc<Vec<u8>>, String> {
    let mut cache = FUENTES.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_default();
    if let Some(data) = cache.get(path) {
        return Ok(data.clone());
    }
    let data = std::fs::read(path)
        .map_err(|e| format!("no se pudo leer la fuente {}: {e}", path.display()))?;
    let data = Arc::new(data);
    cache.insert(path.to_path_buf(), data.clone());
    Ok(data)
}

/// Trazado del texto en mm: línea base en y = 0 empezando en x = 0, y hacia abajo.
/// `alto` es el alto de las mayúsculas. Cada `\n` baja una línea.
pub fn trazado(
    texto: &str,
    fuente_path: &Path,
    alto: f64,
    espaciado: f64,
) -> Result<BezPath, String> {
    let data = fuente(fuente_path)?;
    let face = ttf_parser::Face::parse(&data, 0)
        .map_err(|e| format!("fuente inválida {}: {e}", fuente_path.display()))?;
    let mayus = face
        .capital_height()
        .filter(|&h| h > 0)
        .unwrap_or(face.ascender());
    let escala = alto / f64::from(mayus.max(1));
    let interlineado = f64::from(face.height()) * escala;

    let mut out = Trazador {
        path: BezPath::new(),
        escala,
        origen: Point::ZERO,
    };
    for (n, linea) in texto.lines().enumerate() {
        out.origen = Point::new(0.0, n as f64 * interlineado);
        for c in linea.chars() {
            let Some(g) = face.glyph_index(c) else {
                continue;
            };
            face.outline_glyph(g, &mut out);
            let avance = face.glyph_hor_advance(g).unwrap_or(0);
            out.origen.x += f64::from(avance) * escala + espaciado;
        }
    }
    Ok(out.path)
}

struct Trazador {
    path: BezPath,
    escala: f64,
    origen: Point,
}

impl Trazador {
    fn p(&self, x: f32, y: f32) -> Point {
        // Las fuentes tienen y hacia arriba.
        Point::new(
            self.origen.x + f64::from(x) * self.escala,
            self.origen.y - f64::from(y) * self.escala,
        )
    }
}

impl ttf_parser::OutlineBuilder for Trazador {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.path.move_to(p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.path.line_to(p);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = (self.p(x1, y1), self.p(x, y));
        self.path.quad_to(a, b);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b, c) = (self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        self.path.curve_to(a, b, c);
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}
