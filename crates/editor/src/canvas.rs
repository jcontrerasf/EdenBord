//! Lienzo: dibuja el diseño y maneja zoom, desplazamiento, selección y arrastre.

use std::ops::Range;

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Vec2};
use jef::{Bounds, Jef, StitchKind};

use crate::Editor;

pub(crate) const SELECTION_COLOR: Color32 = Color32::from_rgb(30, 110, 230);
/// Distancia en píxeles para elegir una línea con el clic.
const PICK_PX: f32 = 6.0;

/// Conversión entre coordenadas del diseño y de pantalla.
#[derive(Clone, Copy)]
struct Vista {
    origin: Pos2,
    /// Píxeles por 0,1 mm.
    scale: f32,
}

impl Vista {
    /// Punto en 0,1 mm a pantalla.
    fn p(&self, x: i32, y: i32) -> Pos2 {
        self.origin + Vec2::new(x as f32, y as f32) * self.scale
    }

    /// Punto en mm a pantalla.
    fn mm(&self, p: kurbo::Point) -> Pos2 {
        self.origin + Vec2::new(p.x as f32, p.y as f32) * (10.0 * self.scale)
    }

    fn to_mm(self, pos: Pos2) -> kurbo::Point {
        let v = (pos - self.origin) / (10.0 * self.scale);
        kurbo::Point::new(f64::from(v.x), f64::from(v.y))
    }
}

impl Editor {
    pub(crate) fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, self.fabric);
        if self.doc.is_some() {
            self.canvas_jef(ui, &response, &painter);
        } else if self.proyecto.is_some() {
            self.canvas_proyecto(ui, &response, &painter);
        }
    }

    /// Ajuste inicial, desplazamiento (si no se está moviendo la selección) y zoom.
    fn view_controls(&mut self, ui: &egui::Ui, response: &Response, fit: Bounds) -> Vista {
        let rect = response.rect;
        if self.fit_pending {
            let w = fit.width().max(10) as f32;
            let h = fit.height().max(10) as f32;
            self.scale = 0.9 * (rect.width() / w).min(rect.height() / h);
            let mid = Vec2::new(
                (fit.min_x + fit.max_x) as f32 / 2.0,
                (fit.min_y + fit.max_y) as f32 / 2.0,
            );
            self.pan = -mid * self.scale;
            self.fit_pending = false;
        }
        if !self.moving {
            self.pan += response.drag_delta();
        }

        // Zoom con la rueda o el gesto, centrado en el puntero.
        if response.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = zoom * (scroll / 400.0).exp();
            if factor != 1.0
                && let Some(pointer) = response.hover_pos()
            {
                let anchor = pointer - rect.center() - self.pan;
                self.pan -= anchor * (factor - 1.0);
                self.scale *= factor;
            }
        }
        Vista {
            origin: rect.center() + self.pan,
            scale: self.scale,
        }
    }

    /// Shift + arrastrar mueve la selección. Devuelve si empieza un arrastre (para guardar el
    /// punto de deshacer) y el desplazamiento en píxeles de este cuadro.
    fn selection_drag(&mut self, ui: &egui::Ui, response: &Response) -> (bool, Vec2) {
        let shift = ui.input(|i| i.modifiers.shift);
        let started = response.drag_started() && shift;
        if started {
            self.drag_rest = Vec2::ZERO;
            self.moving = true;
        }
        if response.drag_stopped() {
            self.moving = false;
        }
        let delta = if self.moving && response.dragged() {
            response.drag_delta()
        } else {
            Vec2::ZERO
        };
        (started, delta)
    }

    fn canvas_jef(&mut self, ui: &egui::Ui, response: &Response, painter: &egui::Painter) {
        let (started, delta) = self.selection_drag(ui, response);
        let doc = self.doc.as_mut().unwrap();
        let fit = doc.jef.bounds();
        if started {
            doc.checkpoint();
        }
        if delta != Vec2::ZERO {
            let total = self.drag_rest + delta / self.scale;
            let (dx, dy) = (total.x.trunc(), total.y.trunc());
            self.drag_rest = total - Vec2::new(dx, dy);
            if dx != 0.0 || dy != 0.0 {
                let sel = doc.selection();
                doc.jef.translate(&sel, dx as i32, dy as i32);
                doc.sync();
            }
        }
        let vista = self.view_controls(ui, response, fit);
        let doc = self.doc.as_ref().unwrap();
        let jef = &doc.jef;

        if self.show_hoop {
            draw_hoop(painter, vista, jef);
        }
        let progress = self.progress.min(jef.stitches.len());
        draw_stitches(
            painter,
            vista,
            jef,
            &doc.blocks,
            |b| doc.visible[b],
            progress,
            self.show_jumps,
        );

        // Marco de la selección.
        let sel = doc.selection();
        if !sel.is_empty()
            && let Some(b) = jef.selection_bounds(&sel)
        {
            let r = Rect::from_two_pos(vista.p(b.min_x, b.min_y), vista.p(b.max_x, b.max_y));
            draw_selection_frame(painter, r);
        }
        draw_needle(painter, vista, jef, progress);
    }

    fn canvas_proyecto(&mut self, ui: &egui::Ui, response: &Response, painter: &egui::Painter) {
        let (started, delta) = self.selection_drag(ui, response);
        let doc = self.proyecto.as_mut().unwrap();
        let fit = doc.bounds();
        if started {
            doc.checkpoint();
        }
        let vista = self.view_controls(ui, response, fit);
        let doc = self.proyecto.as_mut().unwrap();
        if delta != Vec2::ZERO {
            let d = delta / (10.0 * vista.scale);
            let sel = doc.selection();
            doc.translate(&sel, f64::from(d.x), f64::from(d.y));
        }

        // Clic: elegir objeto; Ctrl+clic: agregar o quitar de la selección.
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let tol = f64::from(PICK_PX / (10.0 * vista.scale));
            let hit = doc.object_at(vista.to_mm(pos), tol);
            let ctrl = ui.input(|i| i.modifiers.command);
            match (hit, ctrl) {
                (Some(i), true) => doc.seleccion[i] = !doc.seleccion[i],
                (Some(i), false) => {
                    doc.seleccion.fill(false);
                    doc.seleccion[i] = true;
                }
                (None, false) => doc.seleccion.fill(false),
                (None, true) => {}
            }
        }

        let doc = self.proyecto.as_ref().unwrap();
        let jef = &doc.generado.jef;
        if self.show_hoop {
            draw_hoop(painter, vista, jef);
        }
        let progress = self.progress.min(jef.stitches.len());
        let blocks = jef.color_blocks();
        draw_stitches(
            painter,
            vista,
            jef,
            &blocks,
            |_| true,
            progress,
            self.show_jumps,
        );

        // Contornos: guías siempre; los demás solo si se piden o están seleccionados.
        let guide = Stroke::new(1.0, Color32::from_rgba_unmultiplied(30, 30, 30, 200));
        let outline = Stroke::new(1.0, Color32::from_rgba_unmultiplied(30, 30, 30, 110));
        let selected = Stroke::new(2.0, SELECTION_COLOR);
        for (i, obj) in doc.proyecto.objetos.iter().enumerate() {
            let stroke = if doc.seleccion[i] {
                selected
            } else if !obj.visible {
                continue;
            } else if obj.puntada == bordado::Puntada::Ninguna {
                guide
            } else if self.show_outlines {
                outline
            } else {
                continue;
            };
            for pl in &doc.lineas[i] {
                let mut pts: Vec<Pos2> = pl.puntos.iter().map(|&p| vista.mm(p)).collect();
                if pl.cerrada {
                    pts.push(pts[0]);
                }
                painter.add(Shape::line(pts, stroke));
            }
        }
        if let Some(r) = doc.selection_rect() {
            let r = Rect::from_two_pos(vista.mm(r.origin()), vista.mm((r.x1, r.y1).into()));
            draw_selection_frame(painter, r);
        }
        draw_needle(painter, vista, jef, progress);
    }
}

fn draw_hoop(painter: &egui::Painter, vista: Vista, jef: &Jef) {
    let Some((hw, hh)) = jef.header.hoop.size() else {
        return;
    };
    let color = if jef.fits_in(jef.header.hoop) {
        Color32::from_gray(90)
    } else {
        Color32::RED
    };
    let hoop = Rect::from_two_pos(vista.p(-hw / 2, -hh / 2), vista.p(hw / 2, hh / 2));
    painter.rect_stroke(hoop, 4.0, Stroke::new(1.5, color), egui::StrokeKind::Middle);
    let thin = Stroke::new(1.0, color);
    painter.line_segment([vista.p(-30, 0), vista.p(30, 0)], thin);
    painter.line_segment([vista.p(0, -30), vista.p(0, 30)], thin);
}

/// Dibuja las primeras `progress` entradas de los bloques visibles.
fn draw_stitches(
    painter: &egui::Painter,
    vista: Vista,
    jef: &Jef,
    blocks: &[Range<usize>],
    visible: impl Fn(usize) -> bool,
    progress: usize,
    show_jumps: bool,
) {
    // Grosor aproximado de un hilo (0,4 mm), con un mínimo para que se vea al alejar.
    let width = (4.0 * vista.scale).clamp(0.6, 6.0);
    let jump_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(40, 40, 40, 160));
    let mut shapes = Vec::new();
    for (b, block) in blocks.iter().enumerate() {
        if !visible(b) || block.start >= progress {
            continue;
        }
        let [r, g, bl] = jef.thread_rgb(b);
        let stroke = Stroke::new(width, Color32::from_rgb(r, g, bl));
        let end = block.end.min(progress);
        let mut run: Vec<Pos2> = Vec::new();
        for i in block.start..end {
            let s = jef.stitches[i];
            let p = vista.p(s.x, s.y);
            if s.kind == StitchKind::Normal {
                if run.is_empty() {
                    let prev = i.checked_sub(1).map_or((0, 0), |k| {
                        let s = jef.stitches[k];
                        (s.x, s.y)
                    });
                    run.push(vista.p(prev.0, prev.1));
                }
                run.push(p);
            } else {
                if run.len() > 1 {
                    shapes.push(Shape::line(std::mem::take(&mut run), stroke));
                }
                run.clear();
                if show_jumps && s.kind == StitchKind::Jump && i > 0 {
                    let prev = jef.stitches[i - 1];
                    shapes.extend(Shape::dashed_line(
                        &[vista.p(prev.x, prev.y), p],
                        jump_stroke,
                        4.0,
                        3.0,
                    ));
                }
            }
        }
        if run.len() > 1 {
            shapes.push(Shape::line(run, stroke));
        }
    }
    painter.extend(shapes);
}

fn draw_selection_frame(painter: &egui::Painter, r: Rect) {
    let r = r.expand(4.0);
    let stroke = Stroke::new(1.5, SELECTION_COLOR);
    let c = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    painter.extend(Shape::dashed_line(&c, stroke, 6.0, 4.0));
}

/// Marca de la aguja durante la simulación.
fn draw_needle(painter: &egui::Painter, vista: Vista, jef: &Jef, progress: usize) {
    if progress > 0 && progress < jef.stitches.len() {
        let s = jef.stitches[progress - 1];
        painter.circle_stroke(vista.p(s.x, s.y), 6.0, Stroke::new(2.0, Color32::RED));
    }
}
