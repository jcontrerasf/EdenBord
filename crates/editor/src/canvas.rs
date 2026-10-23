//! Lienzo: dibuja el diseño y maneja zoom, desplazamiento, selección y arrastre.

use std::ops::Range;

use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Vec2,
};
use jef::{Bounds, Jef, StitchKind};

use crate::{Editor, Tool};

pub(crate) const SELECTION_COLOR: Color32 = Color32::from_rgb(30, 110, 230);
const PEN_COLOR: Color32 = Color32::from_rgb(220, 0, 140);
const RULER_COLOR: Color32 = Color32::from_rgb(210, 90, 0);
/// Pasos posibles de la cuadrícula, en mm.
const GRID_STEPS: [f64; 5] = [1.0, 5.0, 10.0, 50.0, 100.0];
/// Separación mínima en pantalla entre líneas de la cuadrícula.
const GRID_MIN_PX: f32 = 8.0;
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
        if self.doc.is_some() || self.proyecto.is_some() {
            self.toolbar(ui);
        }
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, self.fabric);
        if self.doc.is_some() {
            self.canvas_jef(ui, &response, &painter);
        } else if self.proyecto.is_some() {
            self.canvas_proyecto(ui, &response, &painter);
        }
    }

    /// Herramientas, cuadrícula y posición del puntero, sobre el lienzo.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let before = self.tool;
        egui::Frame::NONE
            .fill(ui.visuals().panel_fill)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tool, Tool::Select, "🖱 Seleccionar");
                    if self.proyecto.is_some() {
                        ui.selectable_value(&mut self.tool, Tool::Pen, "✒ Pluma");
                    }
                    ui.selectable_value(&mut self.tool, Tool::Ruler, "📏 Regla")
                        .on_hover_text(
                            "Arrastrar para medir · Shift: múltiplos de 45° · Esc: borrar",
                        );
                    ui.separator();
                    ui.checkbox(&mut self.show_grid, "# Cuadrícula");
                    if let Some(p) = self.pointer_mm {
                        ui.separator();
                        ui.monospace(format!("x {:7.1}  y {:7.1} mm", p.x, p.y));
                    }
                });
            });
        if before == Tool::Ruler && self.tool != Tool::Ruler {
            self.ruler = None;
        }
    }

    /// Si la herramienta usa el arrastre principal (entonces no mueve la vista ni la selección).
    fn tool_drags(&self) -> bool {
        match self.tool {
            Tool::Select => false,
            Tool::Pen => self.proyecto.is_some(),
            Tool::Ruler => true,
        }
    }

    /// Arrastrar fija el inicio de la medida en donde se apretó y el fin en el puntero.
    fn ruler_input(&mut self, ui: &egui::Ui, response: &Response, vista: Vista) {
        use egui::PointerButton::Primary;
        if response.drag_started_by(Primary)
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            let a = vista.to_mm(o);
            self.ruler = Some((a, a));
        }
        if response.dragged_by(Primary)
            && let (Some((a, _)), Some(pos)) = (self.ruler, response.interact_pointer_pos())
        {
            let mut b = vista.to_mm(pos);
            if ui.input(|i| i.modifiers.shift) {
                b = snap45(a, b);
            }
            self.ruler = Some((a, b));
        }
    }

    /// Lo común a ambos lienzos después de las puntadas: regla y posición del puntero.
    fn overlays(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        painter: &egui::Painter,
        vista: Vista,
    ) {
        self.pointer_mm = response.hover_pos().map(|p| vista.to_mm(p));
        if self.tool == Tool::Ruler {
            self.ruler_input(ui, response, vista);
        }
        if let Some((a, b)) = self.ruler {
            draw_ruler(painter, vista, a, b);
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
        // Con la pluma o la regla, el arrastre principal es de la herramienta: no mueve la vista.
        let tool_drag = self.tool_drags() && response.dragged_by(egui::PointerButton::Primary);
        if !self.moving && !tool_drag {
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
        let started = response.drag_started() && shift && !self.tool_drags();
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
        if self.show_grid {
            draw_grid(painter, vista, response.rect);
        }
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

    /// Rectángulo (mm) de la imagen de fondo, si está cargada.
    fn fondo_rect(&self) -> Option<kurbo::Rect> {
        let fondo = self.proyecto.as_ref()?.proyecto.fondo.as_ref()?;
        let tex = self.fondo_tex.as_ref()?.1.as_ref()?;
        let [w, h] = tex.size();
        let alto = fondo.ancho * h as f64 / w.max(1) as f64;
        let c = kurbo::Point::new(fondo.centro[0], fondo.centro[1]);
        Some(kurbo::Rect::from_center_size(c, (fondo.ancho, alto)))
    }

    /// Clic agrega un vértice; arrastrar lo agrega y curva; sobre el primero, cierra la zona.
    fn pen_input(&mut self, ui: &egui::Ui, response: &Response, vista: Vista) {
        use egui::PointerButton::Primary;
        let tol = f64::from(PICK_PX / (10.0 * vista.scale));
        let press = if response.drag_started_by(Primary) {
            ui.input(|i| i.pointer.press_origin())
        } else if response.clicked_by(Primary) {
            response.interact_pointer_pos()
        } else {
            None
        };
        if let Some(pos) = press {
            let p = vista.to_mm(pos);
            let on_first = self
                .pen
                .nodos
                .first()
                .is_some_and(|n| n.p.distance(p) < tol);
            if on_first && self.pen.cerrable() {
                self.finish_pen(true);
            } else {
                self.pen.agregar(p);
                self.pen.arrastrando = response.drag_started_by(Primary);
            }
        }
        if self.pen.arrastrando
            && response.dragged_by(Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.pen.manija(vista.to_mm(pos));
        }
        if response.drag_stopped() {
            self.pen.arrastrando = false;
        }
    }

    fn draw_pen(&self, painter: &egui::Painter, response: &Response, vista: Vista) {
        let stroke = Stroke::new(2.0, PEN_COLOR);
        for pl in bordado::geom::polilineas(&self.pen.trazado(false)) {
            let pts: Vec<Pos2> = pl.puntos.iter().map(|&p| vista.mm(p)).collect();
            painter.add(Shape::line(pts, stroke));
        }
        if let (Some(last), Some(hover)) = (self.pen.nodos.last(), response.hover_pos())
            && !self.pen.arrastrando
        {
            painter.extend(Shape::dashed_line(
                &[vista.mm(last.p), hover],
                Stroke::new(1.0, PEN_COLOR),
                4.0,
                3.0,
            ));
        }
        for (i, n) in self.pen.nodos.iter().enumerate() {
            let p = vista.mm(n.p);
            if i == 0 && self.pen.cerrable() {
                painter.circle_stroke(p, PICK_PX, stroke);
            }
            painter.rect_filled(Rect::from_center_size(p, Vec2::splat(6.0)), 0.0, PEN_COLOR);
            if n.h != kurbo::Vec2::ZERO {
                let (a, b) = (vista.mm(n.p - n.h), vista.mm(n.p + n.h));
                painter.line_segment([a, b], Stroke::new(1.0, PEN_COLOR));
                painter.circle_filled(a, 3.0, PEN_COLOR);
                painter.circle_filled(b, 3.0, PEN_COLOR);
            }
        }
    }

    fn canvas_proyecto(&mut self, ui: &egui::Ui, response: &Response, painter: &egui::Painter) {
        let (started, delta) = self.selection_drag(ui, response);
        let mut fit = self.proyecto.as_ref().unwrap().bounds();
        if let Some(r) = self.fondo_rect() {
            fit.min_x = fit.min_x.min((r.x0 * 10.0) as i32);
            fit.min_y = fit.min_y.min((r.y0 * 10.0) as i32);
            fit.max_x = fit.max_x.max((r.x1 * 10.0) as i32);
            fit.max_y = fit.max_y.max((r.y1 * 10.0) as i32);
        }
        if started {
            self.proyecto.as_mut().unwrap().checkpoint();
        }
        let vista = self.view_controls(ui, response, fit);
        if self.tool == Tool::Pen {
            self.pen_input(ui, response, vista);
        }
        let doc = self.proyecto.as_mut().unwrap();
        if delta != Vec2::ZERO {
            let d = delta / (10.0 * vista.scale);
            let sel = doc.selection();
            doc.translate(&sel, f64::from(d.x), f64::from(d.y));
        }

        // Clic: elegir objeto; Ctrl+clic: agregar o quitar de la selección.
        if self.tool == Tool::Select
            && response.clicked()
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
        if let Some(r) = self.fondo_rect()
            && doc.proyecto.fondo.as_ref().is_some_and(|f| f.visible)
            && let Some((_, Some(tex))) = &self.fondo_tex
        {
            let alpha = doc.proyecto.fondo.as_ref().map_or(1.0, |f| f.opacidad);
            let screen = Rect::from_two_pos(vista.mm(r.origin()), vista.mm((r.x1, r.y1).into()));
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(tex.id(), screen, uv, Color32::WHITE.gamma_multiply(alpha));
        }
        if self.show_grid {
            draw_grid(painter, vista, response.rect);
        }
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
        if self.tool == Tool::Pen {
            self.draw_pen(painter, response, vista);
        }
        self.overlays(ui, response, painter, vista);
    }
}

/// Paso de la cuadrícula (mm) para una escala en píxeles por mm.
fn grid_step(px_per_mm: f32) -> f64 {
    GRID_STEPS
        .into_iter()
        .find(|&s| s as f32 * px_per_mm >= GRID_MIN_PX)
        .unwrap_or(GRID_STEPS[GRID_STEPS.len() - 1])
}

/// Cuadrícula en mm alineada al origen (centro del bastidor): líneas finas en cada paso,
/// marcadas cada 10 mm (o cada 5 pasos si el paso es grande) y los ejes más oscuros.
fn draw_grid(painter: &egui::Painter, vista: Vista, rect: Rect) {
    let step = grid_step(10.0 * vista.scale);
    let major = if step < 10.0 { 10.0 } else { step * 5.0 };
    let minor = Stroke::new(1.0, Color32::from_black_alpha(22));
    let strong = Stroke::new(1.0, Color32::from_black_alpha(55));
    let axis = Stroke::new(1.0, Color32::from_black_alpha(100));
    let (lo, hi) = (
        vista.to_mm(rect.left_top()),
        vista.to_mm(rect.right_bottom()),
    );
    let stroke = |v: f64| {
        let k = (v / step).round() as i64;
        if k == 0 {
            axis
        } else if (v / major).round() * major == v {
            strong
        } else {
            minor
        }
    };
    let mut shapes = Vec::new();
    for k in (lo.x / step).ceil() as i64..=(hi.x / step).floor() as i64 {
        let x = vista.mm(kurbo::Point::new(k as f64 * step, 0.0)).x;
        let s = stroke(k as f64 * step);
        shapes.push(Shape::line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            s,
        ));
    }
    for k in (lo.y / step).ceil() as i64..=(hi.y / step).floor() as i64 {
        let y = vista.mm(kurbo::Point::new(0.0, k as f64 * step)).y;
        let s = stroke(k as f64 * step);
        shapes.push(Shape::line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            s,
        ));
    }
    painter.extend(shapes);
    painter.text(
        rect.left_bottom() + Vec2::new(8.0, -8.0),
        Align2::LEFT_BOTTOM,
        format!("cuadrícula {step} mm"),
        FontId::proportional(12.0),
        Color32::from_black_alpha(140),
    );
}

/// Distancia, Δx, Δy (mm) y ángulo (grados; 0 = horizontal, positivo = horario en pantalla).
fn measure(a: kurbo::Point, b: kurbo::Point) -> (f64, f64, f64, f64) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    (dx.hypot(dy), dx, dy, dy.atan2(dx).to_degrees())
}

/// Lleva `b` a la dirección múltiplo de 45° más cercana vista desde `a`.
fn snap45(a: kurbo::Point, b: kurbo::Point) -> kurbo::Point {
    let v = b - a;
    let octavo = std::f64::consts::FRAC_PI_4;
    let ang = (v.y.atan2(v.x) / octavo).round() * octavo;
    let dir = kurbo::Vec2::new(ang.cos(), ang.sin());
    a + dir * v.dot(dir)
}

fn draw_ruler(painter: &egui::Painter, vista: Vista, a: kurbo::Point, b: kurbo::Point) {
    let (pa, pb) = (vista.mm(a), vista.mm(b));
    let stroke = Stroke::new(2.0, RULER_COLOR);
    painter.line_segment([pa, pb], stroke);
    // Marcas perpendiculares en los extremos.
    let dir = (pb - pa).normalized();
    let side = if dir == Vec2::ZERO {
        Vec2::Y
    } else {
        dir.rot90()
    };
    let normal = side * 7.0;
    for p in [pa, pb] {
        painter.line_segment([p - normal, p + normal], stroke);
    }
    let (d, dx, dy, ang) = measure(a, b);
    let text = format!("{d:.1} mm\nΔx {dx:.1}  Δy {dy:.1}  {ang:.1}°");
    let galley = painter.layout_no_wrap(text, FontId::proportional(13.0), Color32::WHITE);
    let pos = pa.lerp(pb, 0.5) + normal * 2.0;
    let rect = Align2::CENTER_CENTER
        .anchor_size(pos, galley.size())
        .expand(4.0);
    painter.rect_filled(rect, 4.0, RULER_COLOR);
    painter.galley(rect.shrink(4.0).min, galley, Color32::WHITE);
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

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;

    #[test]
    fn grid_step_follows_zoom() {
        assert_eq!(grid_step(20.0), 1.0);
        assert_eq!(grid_step(4.0), 5.0);
        assert_eq!(grid_step(1.0), 10.0);
        assert_eq!(grid_step(0.2), 50.0);
        assert_eq!(grid_step(0.01), 100.0);
    }

    #[test]
    fn ruler_measures_and_snaps() {
        let (d, dx, dy, ang) = measure(Point::ZERO, Point::new(3.0, 4.0));
        assert_eq!((d, dx, dy), (5.0, 3.0, 4.0));
        assert!((ang - 53.13).abs() < 0.01);

        let near = |p: Point, q: Point| p.distance(q) < 1e-9;
        assert!(near(
            snap45(Point::ZERO, Point::new(10.0, 1.0)),
            Point::new(10.0, 0.0)
        ));
        assert!(near(
            snap45(Point::ZERO, Point::new(10.0, 9.0)),
            Point::new(9.5, 9.5)
        ));
        let a = Point::new(5.0, 5.0);
        assert!(near(
            snap45(a, Point::new(5.5, -3.0)),
            Point::new(5.0, -3.0)
        ));
    }
}
