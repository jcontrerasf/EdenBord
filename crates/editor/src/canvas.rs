//! Lienzo: dibuja el diseño y maneja zoom, desplazamiento y arrastre de la selección.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use jef::StitchKind;

use crate::Editor;

const SELECTION_COLOR: Color32 = Color32::from_rgb(30, 110, 230);

impl Editor {
    pub(crate) fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, self.fabric);

        let Some(doc) = &mut self.doc else {
            return;
        };

        if self.fit_pending {
            let b = doc.jef.bounds();
            let w = b.width().max(10) as f32;
            let h = b.height().max(10) as f32;
            self.scale = 0.9 * (rect.width() / w).min(rect.height() / h);
            let mid = Vec2::new(
                (b.min_x + b.max_x) as f32 / 2.0,
                (b.min_y + b.max_y) as f32 / 2.0,
            );
            self.pan = -mid * self.scale;
            self.fit_pending = false;
        }

        // Shift + arrastrar mueve la selección; arrastrar sin Shift desplaza la vista.
        let shift = ui.input(|i| i.modifiers.shift);
        if response.drag_started() && shift {
            doc.checkpoint();
            self.drag_rest = Vec2::ZERO;
            self.moving = true;
        }
        if response.drag_stopped() {
            self.moving = false;
        }
        if self.moving && response.dragged() {
            let total = self.drag_rest + response.drag_delta() / self.scale;
            let (dx, dy) = (total.x.trunc(), total.y.trunc());
            self.drag_rest = total - Vec2::new(dx, dy);
            if dx != 0.0 || dy != 0.0 {
                let sel = doc.selection();
                doc.jef.translate(&sel, dx as i32, dy as i32);
                doc.sync();
            }
        } else {
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

        let jef = &doc.jef;
        let origin = rect.center() + self.pan;
        let scale = self.scale;
        let to_screen = |x: i32, y: i32| origin + Vec2::new(x as f32, y as f32) * scale;

        if self.show_hoop
            && let Some((hw, hh)) = jef.header.hoop.size()
        {
            let color = if jef.fits_in(jef.header.hoop) {
                Color32::from_gray(90)
            } else {
                Color32::RED
            };
            let hoop = Rect::from_two_pos(to_screen(-hw / 2, -hh / 2), to_screen(hw / 2, hh / 2));
            painter.rect_stroke(hoop, 4.0, Stroke::new(1.5, color), egui::StrokeKind::Middle);
            let thin = Stroke::new(1.0, color);
            painter.line_segment([to_screen(-30, 0), to_screen(30, 0)], thin);
            painter.line_segment([to_screen(0, -30), to_screen(0, 30)], thin);
        }

        // Grosor aproximado de un hilo (0,4 mm), con un mínimo para que se vea al alejar.
        let width = (4.0 * scale).clamp(0.6, 6.0);
        let jump_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(40, 40, 40, 160));
        let progress = self.progress.min(jef.stitches.len());
        let mut shapes = Vec::new();
        for (b, block) in doc.blocks.iter().enumerate() {
            if !doc.visible[b] || block.start >= progress {
                continue;
            }
            let [r, g, bl] = jef.thread_rgb(b);
            let stroke = Stroke::new(width, Color32::from_rgb(r, g, bl));
            let end = block.end.min(progress);
            let mut run: Vec<Pos2> = Vec::new();
            for i in block.start..end {
                let s = jef.stitches[i];
                let p = to_screen(s.x, s.y);
                if s.kind == StitchKind::Normal {
                    if run.is_empty() {
                        let prev = i.checked_sub(1).map_or((0, 0), |k| {
                            let s = jef.stitches[k];
                            (s.x, s.y)
                        });
                        run.push(to_screen(prev.0, prev.1));
                    }
                    run.push(p);
                } else {
                    if run.len() > 1 {
                        shapes.push(Shape::line(std::mem::take(&mut run), stroke));
                    }
                    run.clear();
                    if self.show_jumps && s.kind == StitchKind::Jump && i > 0 {
                        let prev = jef.stitches[i - 1];
                        shapes.extend(Shape::dashed_line(
                            &[to_screen(prev.x, prev.y), p],
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

        // Marco de la selección.
        let sel = doc.selection();
        if !sel.is_empty()
            && let Some(b) = jef.selection_bounds(&sel)
        {
            let r = Rect::from_two_pos(to_screen(b.min_x, b.min_y), to_screen(b.max_x, b.max_y))
                .expand(4.0);
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

        // Marca de la aguja durante la simulación.
        if progress > 0 && progress < jef.stitches.len() {
            let s = jef.stitches[progress - 1];
            painter.circle_stroke(to_screen(s.x, s.y), 6.0, Stroke::new(2.0, Color32::RED));
        }
    }
}
