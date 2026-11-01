//! Shapes tool: dragging draws the chosen shape ([`Editor::shape`]) in the dragged box
//! (Shift = its natural proportion, Ctrl = from the center); a click places it at its default
//! size. New shapes are guides, like the pen's strokes.

use bordado::figuras::Shape;
use bordado::{Forma, Objeto, Puntada};
use eframe::egui::{self, Pos2, Response, Stroke};
use kurbo::{Affine, Point, Rect, Vec2};

use super::{PEN_COLOR, Vista};
use crate::Editor;

/// Smallest box (mm) a drag has to make; less is taken as a click.
const MIN_MM: f64 = 0.5;

/// The box of a shape dragged from `from` to `to` (mm). `shift` keeps width / height =
/// `ratio`; `ctrl` takes `from` as the center.
pub fn shape_box(from: Point, to: Point, ratio: f64, shift: bool, ctrl: bool) -> Rect {
    let d = to - from;
    let (mut w, mut h) = (d.x.abs(), d.y.abs());
    if shift && ratio > 0.0 {
        if w > h * ratio {
            h = w / ratio;
        } else {
            w = h * ratio;
        }
    }
    let size = Vec2::new(w.copysign(d.x), h.copysign(d.y));
    if ctrl {
        Rect::from_points(from - size, from + size)
    } else {
        Rect::from_points(from, from + size)
    }
}

/// The shape placed in `r` (mm), as an outline.
fn placed(shape: &Shape, r: Rect) -> kurbo::BezPath {
    let mut d = shape.clone().with_size(r.width(), r.height()).path();
    d.apply_affine(Affine::translate(r.center().to_vec2()));
    d
}

impl Editor {
    pub(super) fn shapes_input(&mut self, ui: &egui::Ui, response: &Response, vista: Vista) {
        use egui::PointerButton::Primary;
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if response.drag_started_by(Primary) {
            self.shape_from = ui
                .input(|i| i.pointer.press_origin())
                .map(|p| vista.to_mm(p));
        }
        if response.drag_stopped()
            && let Some(from) = self.shape_from.take()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let r = self.shape_rect(ui, from, vista.to_mm(pos));
            if r.width() >= MIN_MM && r.height() >= MIN_MM {
                self.add_shape(r);
            }
        } else if response.clicked_by(Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            let (s, ratio) = (bordado::figuras::DEFAULT_SIZE, self.shape.ratio());
            let (w, h) = if ratio >= 1.0 {
                (s, s / ratio)
            } else {
                (s * ratio, s)
            };
            let r = Rect::from_center_size(vista.to_mm(pos), (w, h));
            self.add_shape(r);
        }
    }

    fn shape_rect(&self, ui: &egui::Ui, from: Point, to: Point) -> Rect {
        let m = ui.input(|i| i.modifiers);
        shape_box(from, to, self.shape.ratio(), m.shift, m.command)
    }

    fn add_shape(&mut self, r: Rect) {
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let n = doc.proyecto.objetos.len() + 1;
        let forma = Forma::Shape {
            shape: self.shape.clone().with_size(r.width(), r.height()),
            afin: Affine::translate(r.center().to_vec2()).as_coeffs(),
        };
        let nombre = format!("{} {n}", self.shape.name().to_lowercase());
        doc.add(vec![Objeto::new(nombre, 1, forma, Puntada::Ninguna)]);
    }

    /// The shape being dragged.
    pub(super) fn draw_shape_preview(&self, ui: &egui::Ui, painter: &egui::Painter, vista: Vista) {
        let (Some(from), Some(pos)) = (self.shape_from, ui.ctx().pointer_interact_pos()) else {
            return;
        };
        let r = self.shape_rect(ui, from, vista.to_mm(pos));
        let tol = f64::from(0.25 / (10.0 * vista.scale));
        let mut pts: Vec<Pos2> = vec![];
        let stroke = Stroke::new(1.5, PEN_COLOR);
        kurbo::flatten(placed(&self.shape, r).iter(), tol, |el| match el {
            kurbo::PathEl::MoveTo(p) => {
                if pts.len() > 1 {
                    painter.add(egui::Shape::closed_line(std::mem::take(&mut pts), stroke));
                }
                pts = vec![vista.mm(p)];
            }
            kurbo::PathEl::LineTo(p) => pts.push(vista.mm(p)),
            _ => {}
        });
        if pts.len() > 1 {
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_follows_the_drag_and_the_modifiers() {
        let (a, b) = (Point::new(10.0, 10.0), Point::new(4.0, 30.0));
        assert_eq!(
            shape_box(a, b, 1.0, false, false),
            Rect::new(4.0, 10.0, 10.0, 30.0)
        );
        // Shift: the larger side wins, toward the drag.
        assert_eq!(
            shape_box(a, b, 1.0, true, false),
            Rect::new(-10.0, 10.0, 10.0, 30.0)
        );
        assert_eq!(
            shape_box(a, b, 2.0, true, false),
            Rect::new(-30.0, 10.0, 10.0, 30.0)
        );
        // Ctrl: from the center.
        assert_eq!(
            shape_box(a, b, 1.0, false, true),
            Rect::new(4.0, -10.0, 16.0, 30.0)
        );
        assert_eq!(
            shape_box(a, b, 1.0, true, true),
            Rect::new(-10.0, -10.0, 30.0, 30.0)
        );
    }
}
