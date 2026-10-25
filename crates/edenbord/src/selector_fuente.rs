//! Selector de fuente de los textos: familias del sistema con buscador y vista previa, estilo
//! (peso y cursiva) y la opción de usar un archivo propio.

use bordado::fuentes::{self, Fuente};
use bordado::geom::polilineas;
use eframe::egui::{self, Rect, Vec2};

use crate::proyecto::trapecios;
use crate::tema;

/// Lo que se pidió en el selector.
pub enum Pedido {
    Fuente(Fuente),
    /// Elegir un archivo `.ttf`/`.otf`.
    Archivo,
}

/// Alto de la vista previa, en px.
const PREVIA: f32 = 44.0;

/// Controles de la fuente. `muestra` es el texto que se ve en la vista previa.
pub fn selector(ui: &mut egui::Ui, actual: &Fuente, muestra: &str) -> Option<Pedido> {
    let mut pedido = None;
    let id = ui.make_persistent_id("fuente");
    egui::Grid::new("fuente_grid")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Fuente");
            let texto = actual.familia.clone().unwrap_or_else(|| actual.nombre());
            egui::ComboBox::from_id_salt(id)
                .selected_text(texto)
                .width(170.0)
                .height(420.0)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show_ui(ui, |ui| {
                    if let Some(f) = lista(ui, id, actual, muestra) {
                        pedido = Some(Pedido::Fuente(f));
                        ui.close();
                    }
                });
            ui.end_row();

            if let Some(familia) = &actual.familia {
                let estilos = fuentes::estilos(familia);
                if estilos.len() > 1 {
                    ui.label("Estilo");
                    egui::ComboBox::from_id_salt("estilo")
                        .selected_text(fuentes::estilo(actual.peso, actual.cursiva))
                        .show_ui(ui, |ui| {
                            for (peso, cursiva) in estilos {
                                let elegido = peso == actual.peso && cursiva == actual.cursiva;
                                if ui
                                    .selectable_label(elegido, fuentes::estilo(peso, cursiva))
                                    .clicked()
                                {
                                    pedido = Some(Pedido::Fuente(Fuente::sistema(
                                        familia, peso, cursiva,
                                    )));
                                }
                            }
                        });
                    ui.end_row();
                }
            }
            ui.label("");
            if ui
                .small_button("Usar un archivo…")
                .on_hover_text("Una fuente .ttf u .otf que no está instalada")
                .clicked()
            {
                pedido = Some(Pedido::Archivo);
            }
            ui.end_row();
        });
    pedido
}

/// Buscador, vista previa y lista de familias. Devuelve la elegida.
fn lista(ui: &mut egui::Ui, id: egui::Id, actual: &Fuente, muestra: &str) -> Option<Fuente> {
    let id_busqueda = id.with("busqueda");
    let id_previa = id.with("previa");
    let mut busqueda: String = ui.data(|d| d.get_temp(id_busqueda)).unwrap_or_default();
    let campo = ui.add(
        egui::TextEdit::singleline(&mut busqueda)
            .hint_text("Buscar fuente…")
            .desired_width(f32::INFINITY),
    );
    if !campo.has_focus() && busqueda.is_empty() {
        campo.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(id_busqueda, busqueda.clone()));

    // Vista previa de la familia bajo el puntero (o la actual).
    let previa: Option<String> = ui
        .data(|d| d.get_temp(id_previa))
        .or_else(|| actual.familia.clone());
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), PREVIA),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 4.0, tema::LINO_CLARO);
    if let Some(familia) = &previa {
        let f = Fuente::sistema(familia, actual.peso, actual.cursiva);
        dibujar_previa(ui.painter(), rect.shrink(6.0), &f, muestra);
    }

    let filtro = busqueda.to_lowercase();
    let familias: Vec<&String> = fuentes::familias()
        .iter()
        .filter(|f| f.to_lowercase().contains(&filtro))
        .collect();
    if familias.is_empty() {
        ui.weak("Ninguna fuente coincide");
    }
    let mut elegida = None;
    let mut bajo_puntero = None;
    let alto = ui.text_style_height(&egui::TextStyle::Button) + ui.spacing().item_spacing.y;
    egui::ScrollArea::vertical()
        .id_salt("familias")
        .max_height(300.0)
        .show_rows(ui, alto, familias.len(), |ui, filas| {
            for familia in &familias[filas] {
                let marcada = actual.familia.as_ref() == Some(*familia);
                let r = ui.selectable_label(marcada, familia.as_str());
                if r.hovered() {
                    bajo_puntero = Some((*familia).clone());
                }
                if r.clicked() {
                    elegida = Some(Fuente::sistema(familia, actual.peso, actual.cursiva));
                }
            }
        });
    if let Some(f) = bajo_puntero {
        ui.data_mut(|d| d.insert_temp(id_previa, f));
    }
    if elegida.is_some() {
        ui.data_mut(|d| {
            d.remove::<String>(id_busqueda);
            d.remove::<String>(id_previa);
        });
    }
    elegida
}

/// Dibuja `muestra` (su primera línea) con la fuente, ajustada al rectángulo.
fn dibujar_previa(painter: &egui::Painter, rect: Rect, fuente: &Fuente, muestra: &str) {
    let linea = muestra.lines().next().unwrap_or("").trim();
    let linea: String = if linea.is_empty() {
        "Aa Bb Cc".into()
    } else {
        linea.chars().take(24).collect()
    };
    let Ok(d) = bordado::texto::trazado(&linea, fuente, 10.0, 0.0) else {
        painter.text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            "No se puede leer esta fuente",
            egui::FontId::proportional(12.0),
            tema::TEXTO_SUAVE,
        );
        return;
    };
    let caja = kurbo::Shape::bounding_box(&d);
    if caja.width() <= 0.0 || caja.height() <= 0.0 {
        return;
    }
    let escala = (rect.width() / caja.width() as f32).min(rect.height() / caja.height() as f32);
    let origen = rect.left_center() - Vec2::new(0.0, caja.height() as f32 * escala / 2.0);
    let a_pantalla = |p: kurbo::Point| {
        origen + Vec2::new((p.x - caja.x0) as f32, (p.y - caja.y0) as f32) * escala
    };
    let mut mesh = egui::Mesh::default();
    for t in trapecios(&polilineas(&d)) {
        let k = mesh.vertices.len() as u32;
        for p in t {
            mesh.colored_vertex(a_pantalla(p), tema::TEXTO);
        }
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k, k + 2, k + 3);
    }
    painter.add(mesh);
}
