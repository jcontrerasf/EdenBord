//! Paleta de tipos de puntada: una ficha por preset con una muestra cosida de verdad (el preset
//! generado sobre una curva o una zona chica), dibujada con la vista realista.

use std::sync::OnceLock;

use bordado::puntadas::Tramo;
use bordado::{Forma, Grupo, OPACIDAD_GUIA, Objeto, PRESETS, Puntada};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use kurbo::BezPath;

use crate::realista::{self, Texturas};
use crate::tema;

/// Tamaño de la muestra dentro de la ficha, en px.
const MUESTRA: Vec2 = Vec2::new(68.0, 44.0);
/// Las muestras miden esto (mm) a lo ancho, más el ancho de la puntada.
const ANCHO_MM: f64 = 12.0;
/// Formas de muestra, centradas en el origen (mm).
const LINEA: &str = "M-6,2.5 C-3,-6 3,8 6,-2.5";
const ZONA: &str = "M-6,0 C-6,-6 6,-6 6,0 L6,3.5 L-6,3.5 Z";

/// Tramos de cada preset sobre su forma de muestra (se calculan una vez).
fn muestras() -> &'static [Vec<Tramo>] {
    static MUESTRAS: OnceLock<Vec<Vec<Tramo>>> = OnceLock::new();
    MUESTRAS.get_or_init(|| {
        PRESETS
            .iter()
            .map(|p| {
                objeto(p.grupo, p.puntada.clone())
                    .tramos(kurbo::Point::new(-6.0, 0.0))
                    .unwrap_or_default()
            })
            .collect()
    })
}

fn objeto(grupo: Grupo, puntada: Puntada) -> Objeto {
    let zona = grupo == Grupo::Relleno;
    let d = BezPath::from_svg(if zona { ZONA } else { LINEA }).expect("forma de muestra válida");
    Objeto {
        nombre: String::new(),
        visible: true,
        color: 1,
        forma: if zona {
            Forma::Contorno { d }
        } else {
            Forma::Linea { d }
        },
        puntada,
        opacidad: OPACIDAD_GUIA,
        bloqueado: false,
    }
}

/// Muestra la paleta y devuelve el preset elegido. `actual` es la puntada del objeto (la ficha
/// que coincide queda marcada); `color`, el del hilo; sin `zonas`, los rellenos no se eligen.
pub fn paleta(
    ui: &mut egui::Ui,
    texturas: &Texturas,
    actual: &Puntada,
    color: Color32,
    zonas: bool,
) -> Option<Puntada> {
    let mut elegido = None;
    for (grupo, titulo) in [(Grupo::Linea, "Líneas"), (Grupo::Relleno, "Rellenos")] {
        ui.label(egui::RichText::new(titulo).small().color(tema::TEXTO_SUAVE));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for (i, p) in PRESETS.iter().enumerate() {
                // La guía va al principio de las líneas.
                let en_grupo =
                    p.grupo == grupo || (grupo == Grupo::Linea && p.grupo == Grupo::Guia);
                if !en_grupo {
                    continue;
                }
                let activa = grupo != Grupo::Relleno || zonas;
                if ficha(ui, texturas, i, actual, color, activa) {
                    elegido = Some(p.puntada.clone());
                }
            }
        });
        ui.add_space(4.0);
    }
    elegido
}

/// Una ficha: muestra y nombre. Devuelve si se hizo clic.
fn ficha(
    ui: &mut egui::Ui,
    texturas: &Texturas,
    i: usize,
    actual: &Puntada,
    color: Color32,
    activa: bool,
) -> bool {
    let p = &PRESETS[i];
    let alto_texto = 14.0;
    let tam = MUESTRA + Vec2::new(0.0, alto_texto);
    let sense = if activa {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(tam, sense);
    let resp = if activa {
        resp.on_hover_text(p.descripcion)
    } else {
        resp.on_hover_text(format!("{}\nSolo para zonas cerradas", p.descripcion))
    };
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let exacta = *actual == p.puntada;
    let mismo_tipo = actual.tipo() == p.puntada.tipo();
    let borde = if exacta {
        Stroke::new(2.0, tema::COPIHUE)
    } else if resp.hovered() && activa {
        Stroke::new(1.5, tema::TEXTO_SUAVE)
    } else if mismo_tipo {
        Stroke::new(1.0, tema::COPIHUE.gamma_multiply(0.5))
    } else {
        Stroke::new(1.0, tema::BORDE)
    };
    let muestra = Rect::from_min_size(rect.min, MUESTRA);
    let painter = ui.painter_at(rect);
    painter.rect_filled(muestra, 4.0, Color32::from_rgb(214, 208, 196));
    dibujar_muestra(
        &ui.painter_at(muestra.shrink(2.0)),
        texturas,
        i,
        color,
        muestra,
    );
    if !activa {
        painter.rect_filled(muestra, 4.0, tema::LINO.gamma_multiply(0.7));
    }
    painter.rect_stroke(muestra, 4.0, borde, egui::StrokeKind::Inside);
    let texto_color = if activa {
        tema::TEXTO
    } else {
        tema::TEXTO_SUAVE.gamma_multiply(0.6)
    };
    painter.text(
        Pos2::new(rect.center().x, muestra.bottom() + alto_texto / 2.0),
        egui::Align2::CENTER_CENTER,
        p.nombre,
        egui::FontId::proportional(10.5),
        texto_color,
    );
    resp.clicked()
}

fn dibujar_muestra(
    painter: &egui::Painter,
    texturas: &Texturas,
    i: usize,
    color: Color32,
    rect: Rect,
) {
    let px_mm = (rect.width() - 8.0) / (ANCHO_MM as f32 + 3.0);
    let centro = rect.center();
    let pantalla = |p: kurbo::Point| centro + Vec2::new(p.x as f32, p.y as f32) * px_mm;
    let p = &PRESETS[i];
    if p.puntada == Puntada::Ninguna {
        // Guía: la forma punteada.
        let d = BezPath::from_svg(LINEA).expect("forma de muestra válida");
        for pl in bordado::geom::polilineas(&d) {
            let pts: Vec<Pos2> = pl.puntos.iter().map(|&q| pantalla(q)).collect();
            painter.extend(egui::Shape::dashed_line(
                &pts,
                Stroke::new(1.5, color),
                4.0,
                3.0,
            ));
        }
        return;
    }
    let mut bloque = texturas.bloque();
    let ancho = realista::ancho_px(px_mm);
    for tramo in &muestras()[i] {
        for w in tramo.windows(2) {
            bloque.puntada(
                pantalla(w[0]),
                pantalla(w[1]),
                ancho,
                color,
                painter.clip_rect(),
            );
        }
    }
    let mut malla = texturas.malla();
    bloque.cerrar(&mut malla);
    painter.add(malla);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_a_sample_inside_the_tile() {
        for (p, tramos) in PRESETS.iter().zip(muestras()) {
            if p.puntada == Puntada::Ninguna {
                continue;
            }
            assert!(!tramos.is_empty(), "{}", p.nombre);
            for q in tramos.iter().flatten() {
                assert!(q.x.abs() <= 9.0 && q.y.abs() <= 9.0, "{}: {q:?}", p.nombre);
            }
        }
    }
}
