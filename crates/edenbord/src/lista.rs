//! Filas de las listas del panel (bloques de color de un JEF, objetos de un proyecto): elegir
//! con clic y reordenar arrastrando.

use eframe::egui::{self, Align, Layout, Response, Sense, Stroke, Ui, UiBuilder, Vec2};

use crate::tema;

/// Alto de una fila.
const ALTO: f32 = 30.0;

/// Una fila que se elige con clic y se arrastra para reordenar (`i` va como carga del
/// arrastre). `izq` dibuja lo de la izquierda (muestra de color, ícono, nombre) y `der` lo
/// de la derecha, de derecha a izquierda (ojo, candado). Los botones de adentro tienen
/// prioridad sobre el clic de la fila.
pub fn fila(
    ui: &mut Ui,
    i: usize,
    elegida: bool,
    izq: impl FnOnce(&mut Ui),
    der: impl FnOnce(&mut Ui),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ALTO),
        Sense::click_and_drag(),
    );
    let fondo = if elegida {
        Some(ui.visuals().selection.bg_fill)
    } else if resp.hovered() {
        Some(ui.visuals().widgets.hovered.weak_bg_fill)
    } else {
        None
    };
    if let Some(c) = fondo {
        ui.painter().rect_filled(rect, 6.0, c);
    }
    let mut hijo = ui.new_child(
        UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(6.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    hijo.spacing_mut().item_spacing.x = 6.0;
    izq(&mut hijo);
    hijo.with_layout(Layout::right_to_left(Align::Center), der);

    if resp.drag_started() {
        resp.dnd_set_drag_payload(i);
    }
    if resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    resp
}

/// Selección con clic: solo esta fila (o ninguna si ya era la única); con Ctrl/Shift, la
/// agrega o la quita.
pub fn clic(ui: &Ui, sel: &mut [bool], i: usize) {
    if ui.input(|inp| inp.modifiers.command || inp.modifiers.shift) {
        sel[i] = !sel[i];
    } else {
        let sola = sel[i] && sel.iter().filter(|&&s| s).count() == 1;
        sel.fill(false);
        sel[i] = !sola;
    }
}

/// Mientras se arrastra una fila sobre `resp` (la fila `i`): marca dónde caería y devuelve
/// el hueco (0..=n, antes de la fila del mismo número).
pub fn hueco(ui: &Ui, resp: &Response, i: usize) -> Option<usize> {
    resp.dnd_hover_payload::<usize>()?;
    let pos = ui.ctx().pointer_interact_pos()?;
    let r = resp.rect;
    let gap = if pos.y < r.center().y { i } else { i + 1 };
    let y = if gap == i { r.top() } else { r.bottom() };
    ui.painter()
        .hline(r.x_range(), y, Stroke::new(2.0, tema::COPIHUE));
    Some(gap)
}

/// Si se soltó una fila sobre el hueco `gap`: de dónde a dónde moverla.
pub fn soltar(ui: &Ui, gap: Option<usize>) -> Option<(usize, usize)> {
    let gap = gap?;
    if !ui.input(|inp| inp.pointer.any_released()) {
        return None;
    }
    let from = *egui::DragAndDrop::take_payload::<usize>(ui.ctx())?;
    let to = destino(from, gap);
    (to != from).then_some((from, to))
}

/// Índice final de una fila que se saca de `from` y se suelta en el hueco `gap` (antes de la
/// fila `gap`; `n` = al final).
fn destino(from: usize, gap: usize) -> usize {
    if gap > from { gap - 1 } else { gap }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destino_descuenta_la_fila_sacada() {
        assert_eq!(destino(0, 0), 0);
        assert_eq!(destino(0, 1), 0, "justo debajo de sí misma: no se mueve");
        assert_eq!(destino(0, 3), 2, "antes de la fila 3 = queda en 2");
        assert_eq!(destino(3, 0), 0);
        assert_eq!(destino(3, 5), 4, "al final de 5");
    }
}
