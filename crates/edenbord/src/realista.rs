//! Vista realista: cada puntada se dibuja como un hilo con volumen sobre la tela.
//!
//! Cada puntada es una cinta texturada (del ancho de un hilo, con las puntas redondas) más un
//! brillo y una sombra. Las texturas se generan acá (sin archivos) y van juntas en un atlas, así
//! todo el diseño es una sola malla y se respeta el orden: lo cosido después tapa lo de antes.
//!
//! - Hilo: perfil de cilindro (bordes oscuros, centro claro), fibras a lo largo y las puntas más
//!   oscuras, donde el hilo se hunde en la tela. Se multiplica por el color del hilo.
//! - Brillo: blanco con alfa 0, que en egui (alfa premultiplicado) se suma a lo de abajo.
//! - Sombra: mancha difusa corrida hacia abajo a la derecha.

use eframe::egui::{
    self, Color32, ColorImage, Mesh, Pos2, Rect, TextureFilter, TextureHandle, TextureOptions,
    TextureWrapMode, Vec2, pos2,
};

/// Ancho de un hilo de bordar (40 wt), en mm.
pub const ANCHO_HILO: f32 = 0.42;
/// Con hilos más finos que esto (px) la vista realista no se nota: se dibujan líneas.
pub const MINIMO_PX: f32 = 1.5;
/// Sombra: desplazamiento en fracciones del ancho del hilo, y ancho relativo.
const SOMBRA_DESPLAZAMIENTO: Vec2 = Vec2::new(0.3, 0.45);
const SOMBRA_ANCHO: f32 = 1.5;
/// Intensidad del brillo (0–255).
const BRILLO: u8 = 210;

/// Tamaño de cada región del atlas: a lo ancho del hilo y a lo largo.
const ANCHO_TEX: usize = 32;
const LARGO_TEX: usize = 64;
/// Fracción del largo de la textura que ocupa cada punta redonda.
const PUNTA: f32 = 0.25;

/// Regiones del atlas, de izquierda a derecha.
#[derive(Clone, Copy)]
enum Region {
    Hilo = 0,
    Brillo = 1,
    Sombra = 2,
}

/// Lado de un cuadro de la textura de la tela, en mm.
const TELA_MM: f32 = 2.0;
const TELA_TEX: usize = 64;
/// Con menos píxeles por mm que esto no se dibuja el tejido (sería ruido).
const TELA_MIN_PX_MM: f32 = 4.0;

pub struct Texturas {
    atlas: TextureHandle,
    tela: TextureHandle,
}

impl Texturas {
    pub fn new(ctx: &egui::Context) -> Self {
        let atlas = ctx.load_texture("hilo", atlas(), TextureOptions::LINEAR);
        let tela = ctx.load_texture(
            "tela",
            tela(),
            TextureOptions {
                magnification: TextureFilter::Linear,
                minification: TextureFilter::Linear,
                wrap_mode: TextureWrapMode::Repeat,
                mipmap_mode: Some(TextureFilter::Linear),
            },
        );
        Texturas { atlas, tela }
    }

    /// Malla vacía donde se juntan los bloques ([`Bloque::cerrar`]).
    pub fn malla(&self) -> Mesh {
        Mesh::with_texture(self.atlas.id())
    }

    /// Bloque vacío para ir agregando puntadas de un color.
    pub fn bloque(&self) -> Bloque {
        Bloque {
            sombras: self.malla(),
            hilos: self.malla(),
        }
    }

    /// Dibuja el tejido de la tela sobre `rect` (ya pintado con el color de la tela).
    /// `origen` es el (0, 0) del diseño en pantalla, para que el tejido se mueva con la vista.
    pub fn tela(&self, painter: &egui::Painter, rect: Rect, origen: Pos2, px_por_mm: f32) {
        if px_por_mm < TELA_MIN_PX_MM {
            return;
        }
        let lado = TELA_MM * px_por_mm;
        let uv = |p: Pos2| ((p - origen) / lado).to_pos2();
        let mut mesh = Mesh::with_texture(self.tela.id());
        mesh.add_rect_with_uv(
            rect,
            Rect::from_min_max(uv(rect.min), uv(rect.max)),
            Color32::WHITE,
        );
        painter.add(mesh);
    }
}

/// Ancho del hilo en píxeles.
pub fn ancho_px(px_por_mm: f32) -> f32 {
    ANCHO_HILO * px_por_mm
}

/// Puntadas de un color. Las sombras van debajo de todos los hilos del bloque: si cada puntada
/// sombreara a las anteriores, las filas de un relleno cosidas hacia arriba (la sombra cae
/// abajo) se verían más oscuras que las cosidas hacia abajo. Entre colores sí se sombrean: un
/// contorno hace sombra sobre el relleno de antes.
pub struct Bloque {
    sombras: Mesh,
    hilos: Mesh,
}

impl Bloque {
    /// Agrega una puntada de `a` a `b` (pantalla): sombra, hilo y brillo. `ancho` en px.
    /// Las que caen fuera de `recorte` no se agregan.
    pub fn puntada(&mut self, a: Pos2, b: Pos2, ancho: f32, color: Color32, recorte: Rect) {
        puntada(
            &mut self.sombras,
            &mut self.hilos,
            a,
            b,
            ancho,
            color,
            recorte,
        );
    }

    /// Pasa el bloque a la malla (sombras y después hilos) y lo deja vacío.
    pub fn cerrar(&mut self, malla: &mut Mesh) {
        for m in [&mut self.sombras, &mut self.hilos] {
            let tex = m.texture_id;
            malla.append(std::mem::replace(m, Mesh::with_texture(tex)));
        }
    }
}

fn puntada(
    sombras: &mut Mesh,
    mesh: &mut Mesh,
    a: Pos2,
    b: Pos2,
    ancho: f32,
    color: Color32,
    recorte: Rect,
) {
    let r = ancho / 2.0;
    let caja = Rect::from_two_pos(a, b).expand(r * (1.0 + SOMBRA_ANCHO));
    if !caja.intersects(recorte) {
        return;
    }
    let d = b - a;
    let largo = d.length();
    // Una puntada de largo 0 se ve como un punto.
    let dir = if largo > 1e-3 { d / largo } else { Vec2::X };
    let normal = dir.rot90();

    // Sombra: un rectángulo difuso, más ancho y corrido.
    let rs = r * SOMBRA_ANCHO;
    let corrida = Vec2::new(SOMBRA_DESPLAZAMIENTO.x, SOMBRA_DESPLAZAMIENTO.y) * ancho;
    let (sa, sb) = (a + corrida - dir * rs, b + corrida + dir * rs);
    let k = sombras.vertices.len() as u32;
    for (p, v) in [(sa, 0.0), (sb, 1.0)] {
        for (lado, u) in [(-1.0, 0.0), (1.0, 1.0)] {
            sombras.vertices.push(vertice(
                p + normal * (lado * rs),
                Region::Sombra,
                u,
                v,
                Color32::WHITE,
            ));
        }
    }
    cuadro(sombras, k);

    // Hilo y brillo: tira de tres cuadros (punta, cuerpo, punta).
    let brillo = Color32::from_rgba_premultiplied(BRILLO, BRILLO, BRILLO, 0);
    let filas = [
        (a - dir * r, 0.0),
        (a, PUNTA),
        (b, 1.0 - PUNTA),
        (b + dir * r, 1.0),
    ];
    for (region, color) in [(Region::Hilo, color), (Region::Brillo, brillo)] {
        let k = mesh.vertices.len() as u32;
        for (p, v) in filas {
            for (lado, u) in [(-1.0, 0.0), (1.0, 1.0)] {
                mesh.vertices
                    .push(vertice(p + normal * (lado * r), region, u, v, color));
            }
        }
        for f in 0..3 {
            cuadro(mesh, k + 2 * f);
        }
    }
}

fn vertice(pos: Pos2, region: Region, u: f32, v: f32, color: Color32) -> egui::epaint::Vertex {
    // Medio texel adentro, para no tomar color de la región vecina al filtrar.
    let medio = 0.5 / ANCHO_TEX as f32;
    let u = (region as usize as f32 + medio + u * (1.0 - 2.0 * medio)) / 3.0;
    egui::epaint::Vertex {
        pos,
        uv: pos2(u, v),
        color,
    }
}

/// Dos triángulos con los vértices `k..k + 4` (dos filas de dos).
fn cuadro(mesh: &mut Mesh, k: u32) {
    mesh.add_triangle(k, k + 1, k + 2);
    mesh.add_triangle(k + 1, k + 3, k + 2);
}

fn suave(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Coordenadas en la cinta del texel `(i, j)`: `x` de −1 a 1 a lo ancho; `y` = cuánto se metió
/// en una punta (0 en el cuerpo, 1 en el extremo).
fn cinta(i: usize, j: usize) -> (f32, f32) {
    let x = (i as f32 + 0.5) / ANCHO_TEX as f32 * 2.0 - 1.0;
    let v = (j as f32 + 0.5) / LARGO_TEX as f32;
    let y = if v < PUNTA {
        1.0 - v / PUNTA
    } else if v > 1.0 - PUNTA {
        (v - 1.0 + PUNTA) / PUNTA
    } else {
        0.0
    };
    (x, y)
}

/// Texel de cada región (premultiplicado).
fn texel(region: Region, i: usize, j: usize) -> Color32 {
    let (x, y) = cinta(i, j);
    let rho = (x * x + y * y).sqrt();
    // Borde suavizado en unos dos texels.
    let alfa = 1.0 - suave(1.0 - 2.5 / ANCHO_TEX as f32, 1.0, rho);
    match region {
        Region::Hilo => {
            let nz = (1.0 - rho * rho).max(0.0).sqrt();
            let fibras = 1.0 - 0.08 * (0.5 + 0.5 * (x * std::f32::consts::PI * 5.0).sin());
            let hundido = 1.0 - 0.35 * y;
            let luz = (0.42 + 0.58 * nz.powf(0.7)) * fibras * hundido;
            let a = alfa;
            let g = (luz * a * 255.0).round() as u8;
            Color32::from_rgba_premultiplied(g, g, g, (a * 255.0).round() as u8)
        }
        Region::Brillo => {
            let h = 0.45 * (-(x / 0.3).powi(2)).exp() * (1.0 - y).powi(2) * alfa;
            let g = (h * 255.0).round() as u8;
            Color32::from_rgba_premultiplied(g, g, g, 0)
        }
        Region::Sombra => {
            // En la sombra la textura se estira a lo largo: `v` va de 0 a 1 en toda la cinta.
            let v = (j as f32 + 0.5) / LARGO_TEX as f32;
            let a = 0.4
                * (1.0 - suave(0.3, 1.0, x.abs()))
                * (1.0 - suave(0.7, 1.0, (2.0 * v - 1.0).abs()));
            Color32::from_rgba_premultiplied(0, 0, 0, (a * 255.0).round() as u8)
        }
    }
}

fn atlas() -> ColorImage {
    let ancho = 3 * ANCHO_TEX;
    let mut pixels = Vec::with_capacity(ancho * LARGO_TEX);
    for j in 0..LARGO_TEX {
        for region in [Region::Hilo, Region::Brillo, Region::Sombra] {
            pixels.extend((0..ANCHO_TEX).map(|i| texel(region, i, j)));
        }
    }
    ColorImage::new([ancho, LARGO_TEX], pixels)
}

/// Tejido plano: cuadros alternados de hilos verticales y horizontales, como sombra suave
/// (negro con poca opacidad) sobre el color de la tela.
fn tela() -> ColorImage {
    const CUADROS: usize = 4;
    let celda = TELA_TEX / CUADROS;
    let pixels = (0..TELA_TEX * TELA_TEX)
        .map(|k| {
            let (i, j) = (k % TELA_TEX, k / TELA_TEX);
            let vertical = (i / celda + j / celda).is_multiple_of(2);
            // Posición a lo ancho del hilo que está arriba en esta celda (−1 a 1).
            let t = if vertical { i % celda } else { j % celda };
            let x = (t as f32 + 0.5) / celda as f32 * 2.0 - 1.0;
            let sombra = 0.03 + 0.11 * x * x;
            Color32::from_rgba_premultiplied(0, 0, 0, (sombra * 255.0).round() as u8)
        })
        .collect();
    ColorImage::new([TELA_TEX, TELA_TEX], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_is_bright_in_the_middle_and_round_at_the_ends() {
        let centro = texel(Region::Hilo, ANCHO_TEX / 2, LARGO_TEX / 2);
        let borde = texel(Region::Hilo, 1, LARGO_TEX / 2);
        assert_eq!(centro.a(), 255);
        assert!(centro.r() > borde.r());
        // La esquina de la textura queda fuera de la punta redonda.
        assert_eq!(texel(Region::Hilo, 0, 0).a(), 0);
        // El brillo se suma (alfa 0) y está en el centro.
        let b = texel(Region::Brillo, ANCHO_TEX / 2, LARGO_TEX / 2);
        assert!(b.a() == 0 && b.r() > 100);
        assert_eq!(atlas().size, [3 * ANCHO_TEX, LARGO_TEX]);
    }

    #[test]
    fn each_stitch_adds_its_vertices_unless_clipped() {
        let (mut sombras, mut hilos) = (Mesh::default(), Mesh::default());
        let pantalla = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));
        let mut agregar = |a: Pos2, b: Pos2| {
            puntada(&mut sombras, &mut hilos, a, b, 4.0, Color32::RED, pantalla);
            (sombras.vertices.len(), hilos.vertices.len())
        };
        assert_eq!(agregar(pos2(10.0, 10.0), pos2(30.0, 10.0)), (4, 16));
        // Fuera de la pantalla no agrega nada; una puntada de largo 0 sí (es un punto).
        assert_eq!(agregar(pos2(500.0, 10.0), pos2(600.0, 10.0)), (4, 16));
        let p = pos2(50.0, 50.0);
        assert_eq!(agregar(p, p), (8, 32));
        assert_eq!(hilos.indices.len(), 2 * 3 * 2 * (3 + 3));
        // El hilo va de a − r a b + r a lo largo.
        let xs: Vec<f32> = hilos.vertices[..8].iter().map(|v| v.pos.x).collect();
        assert_eq!(xs, vec![8.0, 8.0, 10.0, 10.0, 30.0, 30.0, 32.0, 32.0]);
        assert!(sombras.is_valid() && hilos.is_valid());
    }
}
