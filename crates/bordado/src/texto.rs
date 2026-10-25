//! Texto a trazados con una fuente TrueType/OpenType (sin kerning ni ligaduras).

use kurbo::{BezPath, Point};

use crate::fuentes::{self, Fuente};

/// Trazado del texto en mm: línea base en y = 0 empezando en x = 0, y hacia abajo.
/// `alto` es el alto de las mayúsculas. Cada `\n` baja una línea.
pub fn trazado(texto: &str, fuente: &Fuente, alto: f64, espaciado: f64) -> Result<BezPath, String> {
    componer(texto, fuente, alto, espaciado).map(|(path, _)| path)
}

/// Dónde sigue el texto (para un cursor al escribir): punto en la línea base después del
/// último carácter, en las mismas coordenadas que [`trazado`]. Un `\n` al final ya cuenta
/// como línea nueva.
pub fn fin(texto: &str, fuente: &Fuente, alto: f64, espaciado: f64) -> Result<Point, String> {
    componer(texto, fuente, alto, espaciado).map(|(_, fin)| fin)
}

fn componer(
    texto: &str,
    fuente: &Fuente,
    alto: f64,
    espaciado: f64,
) -> Result<(BezPath, Point), String> {
    let (data, index) = fuentes::datos(fuente)?;
    let face = ttf_parser::Face::parse(&data, index)
        .map_err(|e| format!("fuente inválida «{}»: {e}", fuente.nombre()))?;
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
    for (n, linea) in texto.split('\n').enumerate() {
        let linea = linea.strip_suffix('\r').unwrap_or(linea);
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
    Ok((out.path, out.origen))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_point_follows_the_text() {
        let Some(f) = fuentes::predeterminada() else {
            eprintln!("aviso: el sistema no tiene fuentes; prueba saltada");
            return;
        };
        assert_eq!(fin("", &f, 10.0, 0.0).unwrap(), Point::ZERO);
        let a = fin("Ho", &f, 10.0, 0.0).unwrap();
        let b = fin("Hola", &f, 10.0, 0.0).unwrap();
        assert!(a.y == 0.0 && b.x > a.x && a.x > 0.0);
        let c = fin("Hola\n", &f, 10.0, 0.0).unwrap();
        assert!(c.x == 0.0 && c.y > 10.0, "{c:?}");
        // Más espacio entre letras: termina más a la derecha.
        assert!(fin("Hola", &f, 10.0, 1.0).unwrap().x > b.x + 3.9);
    }
}
