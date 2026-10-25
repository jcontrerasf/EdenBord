//! Lienzo: dibuja el diseño y maneja zoom, desplazamiento, selección y arrastre.

use std::ops::Range;

use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Vec2,
};
use jef::{Bounds, Jef, StitchKind};

use crate::gestos::{self, Blanco, Gesto, Tipo};
use crate::nodos::{self, Lado};
use crate::{Editor, Tool, realista};

pub(crate) const SELECTION_COLOR: Color32 = Color32::from_rgb(30, 110, 230);
const PEN_COLOR: Color32 = Color32::from_rgb(220, 0, 140);
const RULER_COLOR: Color32 = Color32::from_rgb(210, 90, 0);
/// Pasos posibles de la cuadrícula, en mm.
const GRID_STEPS: [f64; 5] = [1.0, 5.0, 10.0, 50.0, 100.0];
/// Separación mínima en pantalla entre líneas de la cuadrícula.
const GRID_MIN_PX: f32 = 8.0;
/// Distancia en píxeles para elegir una línea con el clic.
const PICK_PX: f32 = 6.0;
/// Margen en píxeles entre la selección y su marco.
const FRAME_PX: f32 = 4.0;
/// Zoom mínimo y máximo, en píxeles por 0,1 mm. Más de 60 px/mm no aporta (las puntadas van
/// en 0,1 mm) y las curvas guardadas (aproximadas a `geom::TOLERANCIA`) se verían angulosas.
const MIN_SCALE: f32 = 0.01;
const MAX_SCALE: f32 = 6.0;
/// Distancia en píxeles para tomar una manija.
const HANDLE_PX: f32 = 7.0;
/// Altura en píxeles de la manija de giro sobre el marco.
const ROTATE_PX: f32 = 20.0;
/// Largo en píxeles de la manija del ángulo del relleno, desde el centro del objeto.
const ANGLE_PX: f32 = 40.0;
const ANGLE_COLOR: Color32 = Color32::from_rgb(0xd7, 0x26, 0x3d);

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

    /// Rectángulo en mm a pantalla.
    fn rect(&self, r: kurbo::Rect) -> Rect {
        Rect::from_two_pos(self.mm(r.origin()), self.mm((r.x1, r.y1).into()))
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
        if self.realista && self.texturas.is_none() {
            self.texturas = Some(realista::Texturas::new(ui.ctx()));
        }
        if self.doc.is_some() {
            self.canvas_jef(ui, &response, &painter);
        } else if self.proyecto.is_some() {
            self.canvas_proyecto(ui, &response, &painter);
        }
    }

    /// Texturas de la vista realista, si está activa.
    fn texturas(&self) -> Option<&realista::Texturas> {
        self.texturas.as_ref().filter(|_| self.realista)
    }

    /// Tejido de la tela (solo en la vista realista).
    fn draw_fabric(&self, painter: &egui::Painter, rect: Rect, vista: Vista) {
        if let Some(t) = self.texturas() {
            t.tela(painter, rect, vista.origin, vista.scale * 10.0);
        }
    }

    /// Si la herramienta usa el arrastre principal (entonces no mueve la vista ni la selección).
    fn tool_drags(&self) -> bool {
        match self.tool {
            Tool::Select => false,
            Tool::Pen | Tool::Nodes => self.proyecto.is_some(),
            Tool::Ruler => true,
            Tool::Texto => self.proyecto.is_some(),
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

    /// Ajuste inicial, desplazamiento y zoom. La vista se mueve con el botón central o derecho,
    /// o con Espacio + arrastrar; con `primary_pans` (JEF) también con el arrastre principal.
    fn view_controls(
        &mut self,
        ui: &egui::Ui,
        response: &Response,
        fit: Bounds,
        primary_pans: bool,
    ) -> Vista {
        let rect = response.rect;
        // Recién ajustada y sin tocar: si el lienzo cambia de tamaño (la ventana se acomoda
        // al abrir, o se mueve un panel), se vuelve a ajustar.
        if self.fit_size.is_some_and(|s| s != rect.size()) {
            self.fit_pending = true;
        }
        if self.fit_pending {
            let w = fit.width().max(10) as f32;
            let h = fit.height().max(10) as f32;
            self.scale =
                (0.9 * (rect.width() / w).min(rect.height() / h)).clamp(MIN_SCALE, MAX_SCALE);
            let mid = Vec2::new(
                (fit.min_x + fit.max_x) as f32 / 2.0,
                (fit.min_y + fit.max_y) as f32 / 2.0,
            );
            self.pan = -mid * self.scale;
            self.fit_pending = false;
            self.fit_size = Some(rect.size());
        }
        use egui::PointerButton::{Middle, Primary, Secondary};
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        // Con la pluma o la regla, el arrastre principal es de la herramienta: no mueve la vista.
        let primary = primary_pans && !self.moving && !self.tool_drags();
        if response.dragged_by(Middle)
            || response.dragged_by(Secondary)
            || (response.dragged_by(Primary) && (space || primary))
        {
            self.pan += response.drag_delta();
            self.fit_size = None;
        }

        // Zoom con la rueda o el gesto, centrado en el puntero.
        if response.hovered() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = zoom * (scroll / 400.0).exp();
            let factor = (self.scale * factor).clamp(MIN_SCALE, MAX_SCALE) / self.scale;
            if factor != 1.0
                && let Some(pointer) = response.hover_pos()
            {
                let anchor = pointer - rect.center() - self.pan;
                self.pan -= anchor * (factor - 1.0);
                self.scale *= factor;
                self.fit_size = None;
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
        let vista = self.view_controls(ui, response, fit, true);
        self.draw_fabric(painter, response.rect, vista);
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
            None,
            self.texturas(),
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
        self.overlays(ui, response, painter, vista);
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
        // Aproximada según el zoom (un cuarto de píxel), no con la tolerancia de las puntadas.
        let tol = f64::from(0.25 / (10.0 * vista.scale));
        let mut pts: Vec<Pos2> = vec![];
        kurbo::flatten(self.pen.trazado(false).iter(), tol, |el| match el {
            kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => pts.push(vista.mm(p)),
            _ => {}
        });
        painter.add(Shape::line(pts, stroke));
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
        let mut fit = self.proyecto.as_ref().unwrap().bounds();
        if let Some(r) = self.fondo_rect() {
            fit.min_x = fit.min_x.min((r.x0 * 10.0) as i32);
            fit.min_y = fit.min_y.min((r.y0 * 10.0) as i32);
            fit.max_x = fit.max_x.max((r.x1 * 10.0) as i32);
            fit.max_y = fit.max_y.max((r.y1 * 10.0) as i32);
        }
        let vista = self.view_controls(ui, response, fit, false);
        self.draw_fabric(painter, response.rect, vista);
        match self.tool {
            Tool::Pen => self.pen_input(ui, response, vista),
            Tool::Select => self.select_input(ui, response, vista),
            Tool::Nodes => self.nodes_input(ui, response, vista),
            Tool::Texto => self.text_input(ui, response, vista),
            Tool::Ruler => {}
        }

        let doc = self.proyecto.as_ref().unwrap();
        // Transformación en curso de los objetos seleccionados (se aplica al soltar).
        let preview = match self.gesto {
            Some(Gesto::Transformar {
                blanco: Blanco::Objetos,
                af,
                ..
            }) => Some(af),
            _ => None,
        };
        if let Some(r) = self.fondo_rect_preview()
            && doc.proyecto.fondo.as_ref().is_some_and(|f| f.visible)
            && let Some((_, Some(tex))) = &self.fondo_tex
        {
            let alpha = doc.proyecto.fondo.as_ref().map_or(1.0, |f| f.opacidad);
            let screen = vista.rect(r);
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
        let moved = preview.map(|af| {
            let mut m = vec![false; jef.stitches.len()];
            for i in doc.selection() {
                if let Some(r) = doc.generado.rangos.get(i) {
                    m[r.clone()].fill(true);
                }
            }
            (m, af)
        });
        // Relleno traslúcido de las guías (color de hilo), para ver agujeros y partes.
        let mut mesh = egui::Mesh::default();
        for (i, obj) in doc.proyecto.objetos.iter().enumerate() {
            if !obj.visible || doc.relleno_guias[i].is_empty() {
                continue;
            }
            let [r, g, b] = jef::palette::thread(obj.color).map_or([128; 3], |t| t.rgb);
            let alpha = (obj.opacidad.clamp(0.0, 1.0) * 255.0).round() as u8;
            let color = Color32::from_rgba_unmultiplied(r, g, b, alpha);
            let af = if doc.seleccion[i] {
                preview.unwrap_or(kurbo::Affine::IDENTITY)
            } else {
                kurbo::Affine::IDENTITY
            };
            for t in &doc.relleno_guias[i] {
                let k = mesh.vertices.len() as u32;
                for &p in t {
                    mesh.colored_vertex(vista.mm(af * p), color);
                }
                mesh.add_triangle(k, k + 1, k + 2);
                mesh.add_triangle(k, k + 2, k + 3);
            }
        }
        painter.add(Shape::mesh(mesh));

        draw_stitches(
            painter,
            vista,
            jef,
            &blocks,
            |_| true,
            progress,
            self.show_jumps,
            moved.as_ref().map(|(m, af)| (m.as_slice(), *af)),
            self.texturas(),
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
            let af = if doc.seleccion[i] {
                preview.unwrap_or(kurbo::Affine::IDENTITY)
            } else {
                kurbo::Affine::IDENTITY
            };
            for pl in &doc.lineas[i] {
                let mut pts: Vec<Pos2> = pl.puntos.iter().map(|&p| vista.mm(af * p)).collect();
                if pl.cerrada {
                    pts.push(pts[0]);
                }
                painter.add(Shape::line(pts, stroke));
            }
        }
        if self.tool == Tool::Select
            && let Some((r, completo)) = self.handles_rect(vista)
        {
            draw_handles(painter, r, completo);
        } else if self.tool == Tool::Nodes {
            self.draw_nodes(painter, vista);
        } else if let Some(r) = doc.selection_rect() {
            draw_selection_frame(painter, vista.rect(r).expand(FRAME_PX));
        }
        if let Some((_, centro, punta)) = self.angle_handle(vista) {
            draw_angle_handle(painter, vista.mm(centro), punta);
        }
        draw_needle(painter, vista, jef, progress);
        if self.tool == Tool::Pen {
            self.draw_pen(painter, response, vista);
        }
        self.draw_text_cursor(ui, painter, vista);
        self.draw_gesture(painter, response, vista);
        if let Some(GestoNodo::Caja { a, b, .. }) = self.gesto_nodo {
            draw_box(painter, Rect::from_two_pos(vista.mm(a), vista.mm(b)));
        }
        self.overlays(ui, response, painter, vista);
    }

    /// Rectángulo (mm) del fondo con el gesto en curso aplicado.
    fn fondo_rect_preview(&self) -> Option<kurbo::Rect> {
        let r = self.fondo_rect()?;
        Some(match self.gesto {
            Some(Gesto::Transformar {
                blanco: Blanco::Fondo,
                af,
                ..
            }) => af.transform_rect_bbox(r),
            _ => r,
        })
    }

    /// Lo que tiene manijas (la selección o el fondo): su rectángulo en mm y a qué se aplica.
    fn target_rect(&self) -> Option<(kurbo::Rect, Blanco)> {
        if self.fondo_sel {
            return Some((self.fondo_rect()?, Blanco::Fondo));
        }
        // Con un objeto bloqueado en la selección no hay manijas: no se mueve con el ratón.
        let doc = self.proyecto.as_ref()?;
        if doc
            .selection()
            .iter()
            .any(|&i| doc.proyecto.objetos[i].bloqueado)
        {
            return None;
        }
        Some((doc.selection_rect()?, Blanco::Objetos))
    }

    /// Marco de las manijas en pantalla (con el gesto aplicado) y si lleva lados y giro.
    fn handles_rect(&self, vista: Vista) -> Option<(Rect, bool)> {
        if matches!(self.gesto, Some(Gesto::Caja { .. })) {
            return None;
        }
        let (r, blanco) = self.target_rect()?;
        let r = match self.gesto {
            Some(Gesto::Transformar { af, .. }) => af.transform_rect_bbox(r),
            _ => r,
        };
        Some((vista.rect(r).expand(FRAME_PX), blanco == Blanco::Objetos))
    }

    /// Manija del ángulo del relleno: con un solo objeto elegido (no bloqueado) cuya puntada
    /// tiene ángulo. Devuelve el objeto, el centro (mm) y la punta de la manija en pantalla.
    fn angle_handle(&self, vista: Vista) -> Option<(usize, kurbo::Point, Pos2)> {
        if self.tool != Tool::Select
            || matches!(
                self.gesto,
                Some(Gesto::Transformar { .. } | Gesto::Caja { .. })
            )
        {
            return None;
        }
        let doc = self.proyecto.as_ref()?;
        let [i] = doc.selection()[..] else {
            return None;
        };
        let obj = &doc.proyecto.objetos[i];
        let angulo = obj.puntada.angulo().filter(|_| !obj.bloqueado)?;
        let centro = doc.selection_rect()?.center();
        let a = angulo.to_radians() as f32;
        let punta = vista.mm(centro) + Vec2::new(a.cos(), a.sin()) * ANGLE_PX;
        Some((i, centro, punta))
    }

    /// Qué hay bajo el puntero, en orden de prioridad.
    fn pick(&self, vista: Vista, pos: Pos2) -> Pick {
        if let Some((i, _, punta)) = self.angle_handle(vista)
            && punta.distance(pos) <= HANDLE_PX
        {
            return Pick::Angulo(i);
        }
        if let Some((r, completo)) = self.handles_rect(vista)
            && let Some(tipo) = handles(r, completo)
                .into_iter()
                .find(|(_, p)| p.distance(pos) <= HANDLE_PX)
                .map(|(t, _)| t)
        {
            return Pick::Manija(tipo);
        }
        let doc = self.proyecto.as_ref().unwrap();
        let p = vista.to_mm(pos);
        let tol = f64::from(PICK_PX / (10.0 * vista.scale));
        if let Some(i) = doc.object_at(p, tol) {
            return if doc.seleccion[i] {
                Pick::Seleccion
            } else {
                Pick::Objeto(i)
            };
        }
        if let Some((r, blanco)) = self.target_rect()
            && vista.rect(r).expand(FRAME_PX).contains(pos)
        {
            return match blanco {
                Blanco::Objetos => Pick::Seleccion,
                Blanco::Fondo => Pick::Fondo,
            };
        }
        let fondo_libre = doc
            .proyecto
            .fondo
            .as_ref()
            .is_some_and(|f| f.visible && !f.bloqueado);
        if fondo_libre && self.fondo_rect().is_some_and(|r| r.contains(p)) {
            return Pick::Fondo;
        }
        Pick::Nada
    }

    /// Herramienta de selección: clic elige, arrastrar mueve, las manijas escalan y giran, y
    /// arrastrar en el vacío selecciona con un rectángulo.
    fn select_input(&mut self, ui: &egui::Ui, response: &Response, vista: Vista) {
        use egui::PointerButton::Primary;
        let (space, mods) = ui.input(|i| {
            let m = gestos::Mods {
                shift: i.modifiers.shift,
                ctrl: i.modifiers.command,
            };
            (i.key_down(egui::Key::Space), m)
        });
        let sumar = mods.shift || mods.ctrl;
        // Una selección hecha en el panel deja de lado el fondo.
        if self
            .proyecto
            .as_ref()
            .is_some_and(|d| !d.selection().is_empty())
        {
            self.fondo_sel = false;
        }
        let fondo_libre = self
            .proyecto
            .as_ref()
            .and_then(|d| d.proyecto.fondo.as_ref())
            .is_some_and(|f| f.visible && !f.bloqueado);
        if self.fondo_sel && (!fondo_libre || self.fondo_rect().is_none()) {
            self.fondo_sel = false;
        }

        if space {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        } else if self.gesto.is_none()
            && let Some(pos) = response.hover_pos()
        {
            let icon = match self.pick(vista, pos) {
                Pick::Manija(t) => handle_cursor(t),
                Pick::Angulo(_) => egui::CursorIcon::Grab,
                Pick::Seleccion | Pick::Objeto(_) => egui::CursorIcon::Move,
                Pick::Fondo if self.fondo_sel => egui::CursorIcon::Move,
                _ => egui::CursorIcon::Default,
            };
            ui.ctx().set_cursor_icon(icon);
        }

        if response.drag_started_by(Primary)
            && !space
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            let desde = vista.to_mm(o);
            let pick = self.pick(vista, o);
            if let Pick::Angulo(obj) = pick
                && let Some((_, centro, _)) = self.angle_handle(vista)
            {
                let angulo = self.proyecto.as_ref().unwrap().proyecto.objetos[obj]
                    .puntada
                    .angulo()
                    .unwrap_or(0.0);
                self.gesto = Some(Gesto::Angulo {
                    obj,
                    centro,
                    angulo,
                    inicial: angulo,
                });
                return;
            }
            let doc = self.proyecto.as_mut().unwrap();
            match pick {
                Pick::Objeto(i) => {
                    if !sumar {
                        doc.seleccion.fill(false);
                    }
                    doc.seleccion[i] = true;
                    self.fondo_sel = false;
                }
                Pick::Fondo if !self.fondo_sel => {
                    doc.seleccion.fill(false);
                    self.fondo_sel = true;
                }
                _ => {}
            }
            let tipo = match pick {
                Pick::Manija(t) => Some(t),
                Pick::Seleccion | Pick::Objeto(_) | Pick::Fondo => Some(Tipo::Mover),
                Pick::Nada | Pick::Angulo(_) => None,
            };
            self.gesto = match (tipo, self.target_rect()) {
                (Some(tipo), Some((r0, blanco))) => Some(Gesto::Transformar {
                    blanco,
                    tipo,
                    r0,
                    desde,
                    af: kurbo::Affine::IDENTITY,
                }),
                _ => Some(Gesto::Caja {
                    a: desde,
                    b: desde,
                    sumar,
                }),
            };
        }
        if response.dragged_by(Primary)
            && let (Some(g), Some(pos)) = (&mut self.gesto, response.interact_pointer_pos())
        {
            g.arrastrar(vista.to_mm(pos), mods);
            // El ángulo se aplica en vivo: se ve el relleno nuevo mientras se arrastra.
            if let Gesto::Angulo { obj, angulo, .. } = *g
                && let Some(doc) = self.proyecto.as_mut()
                && doc.proyecto.objetos[obj].puntada.angulo() != Some(angulo)
            {
                doc.edit_grouped(format!("angulo {obj}"), |p| {
                    if let Some(a) = p.objetos[obj].puntada.angulo_mut() {
                        *a = angulo;
                    }
                });
            }
        }
        if response.drag_stopped() {
            self.finish_gesture();
        }

        // Clic: elegir objeto (o el fondo); Ctrl o Shift: agregar o quitar de la selección.
        if response.clicked_by(Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let pick = self.pick(vista, pos)
            && !matches!(pick, Pick::Manija(_) | Pick::Angulo(_))
        {
            let doc = self.proyecto.as_mut().unwrap();
            let p = vista.to_mm(pos);
            let tol = f64::from(PICK_PX / (10.0 * vista.scale));
            match (doc.object_at(p, tol), sumar) {
                (Some(i), true) => doc.seleccion[i] = !doc.seleccion[i],
                (Some(i), false) => {
                    doc.seleccion.fill(false);
                    doc.seleccion[i] = true;
                }
                (None, false) => {
                    doc.seleccion.fill(false);
                    self.fondo_sel = pick == Pick::Fondo;
                }
                (None, true) => {}
            }
        }
    }

    /// Aplica el gesto terminado (un solo punto de deshacer).
    pub(crate) fn finish_gesture(&mut self) {
        let (Some(g), Some(doc)) = (self.gesto.take(), self.proyecto.as_mut()) else {
            return;
        };
        match g {
            Gesto::Transformar {
                blanco: Blanco::Objetos,
                af,
                ..
            } => {
                let sel = doc.selection();
                doc.transform_affine(&sel, af);
            }
            Gesto::Transformar {
                blanco: Blanco::Fondo,
                af,
                r0,
                ..
            } => {
                if af != kurbo::Affine::IDENTITY {
                    let r = af.transform_rect_bbox(r0);
                    doc.edit_fondo(|f| {
                        f.centro = [r.center().x, r.center().y];
                        f.ancho = r.width();
                    });
                }
            }
            // Ya se aplicó mientras se arrastraba.
            Gesto::Angulo { .. } => {}
            Gesto::Caja { a, b, sumar } => {
                let dentro = doc.objects_in(kurbo::Rect::from_points(a, b));
                if !sumar {
                    doc.seleccion.fill(false);
                }
                for &i in &dentro {
                    doc.seleccion[i] = true;
                }
                if !dentro.is_empty() || !sumar {
                    self.fondo_sel = false;
                }
            }
        }
    }

    /// Nodos del objeto en edición (`None` si no hay uno editable) y limpia la selección de
    /// nodos si cambió el objeto o dejaron de existir.
    fn edited_nodes(&mut self) -> Option<(usize, Vec<nodos::Sub>)> {
        let doc = self.proyecto.as_ref()?;
        let editable = doc.editable();
        if editable != self.nodos_obj {
            self.nodos_obj = editable;
            self.nodos_sel.clear();
        }
        let i = editable?;
        let subs = doc.nodes(i)?;
        self.nodos_sel
            .retain(|&id| nodos::nodo(&subs, id).is_some());
        Some((i, subs))
    }

    /// Nodo o manija (de un nodo seleccionado) bajo el puntero.
    fn node_at(&self, subs: &[nodos::Sub], vista: Vista, pos: Pos2) -> Option<NodeHit> {
        for &id in &self.nodos_sel {
            let n = nodos::nodo(subs, id)?;
            for (lado, q) in [(Lado::Entrada, n.entrada), (Lado::Salida, n.salida)] {
                if q.distance(n.p) > 1e-6 && vista.mm(q).distance(pos) <= HANDLE_PX {
                    return Some(NodeHit::Manija(id, lado));
                }
            }
        }
        nodos::todos(subs)
            .filter_map(|id| Some((id, vista.mm(nodos::nodo(subs, id)?.p).distance(pos))))
            .filter(|&(_, d)| d <= HANDLE_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| NodeHit::Nodo(id))
    }

    /// Herramienta de nodos: clic elige nodo (Shift/Ctrl agrega), arrastrar mueve nodos o
    /// manijas, arrastrar en el vacío selecciona nodos con un rectángulo, doble clic en el
    /// trazo agrega un nodo. Clic en otro objeto lo elige para editarlo.
    fn nodes_input(&mut self, ui: &egui::Ui, response: &Response, vista: Vista) {
        use egui::PointerButton::Primary;
        let (space, sumar) = ui.input(|i| {
            (
                i.key_down(egui::Key::Space),
                i.modifiers.shift || i.modifiers.command,
            )
        });
        let edited = self.edited_nodes();
        let hit = |s: &Self, pos: Pos2| {
            edited
                .as_ref()
                .and_then(|(_, subs)| s.node_at(subs, vista, pos))
        };
        if !space
            && self.gesto_nodo.is_none()
            && let Some(pos) = response.hover_pos()
            && hit(self, pos).is_some()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }

        if response.drag_started_by(Primary)
            && !space
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            let desde = vista.to_mm(o);
            self.gesto_nodo = match hit(self, o) {
                Some(NodeHit::Manija(id, lado)) => {
                    self.proyecto.as_mut().unwrap().checkpoint();
                    Some(GestoNodo::Manija { id, lado })
                }
                Some(NodeHit::Nodo(id)) => {
                    if !self.nodos_sel.contains(&id) {
                        if !sumar {
                            self.nodos_sel.clear();
                        }
                        self.nodos_sel.push(id);
                    }
                    self.proyecto.as_mut().unwrap().checkpoint();
                    Some(GestoNodo::Mover { ultimo: desde })
                }
                None => Some(GestoNodo::Caja {
                    a: desde,
                    b: desde,
                    sumar,
                }),
            };
        }
        if response.dragged_by(Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Some((i, mut subs)) = edited.clone()
        {
            let p = vista.to_mm(pos);
            let doc = self.proyecto.as_mut().unwrap();
            match &mut self.gesto_nodo {
                Some(GestoNodo::Mover { ultimo }) => {
                    nodos::mover(&mut subs, &self.nodos_sel, p - *ultimo);
                    *ultimo = p;
                    doc.set_nodes_live(i, &subs);
                }
                Some(GestoNodo::Manija { id, lado }) => {
                    nodos::mover_manija(&mut subs, *id, *lado, p);
                    doc.set_nodes_live(i, &subs);
                }
                Some(GestoNodo::Caja { b, .. }) => *b = p,
                None => {}
            }
        }
        if let Some(GestoNodo::Caja { b, .. }) = &mut self.gesto_nodo
            && edited.is_none()
            && let Some(pos) = response.interact_pointer_pos()
        {
            *b = vista.to_mm(pos);
        }
        if response.drag_stopped()
            && let Some(g) = self.gesto_nodo.take()
        {
            match g {
                GestoNodo::Mover { .. } | GestoNodo::Manija { .. } => {
                    self.proyecto.as_mut().unwrap().sync();
                }
                GestoNodo::Caja { a, b, sumar } => {
                    let r = kurbo::Rect::from_points(a, b);
                    if !sumar {
                        self.nodos_sel.clear();
                    }
                    if let Some((_, subs)) = &edited {
                        for id in nodos::todos(subs) {
                            if nodos::nodo(subs, id).is_some_and(|n| r.contains(n.p))
                                && !self.nodos_sel.contains(&id)
                            {
                                self.nodos_sel.push(id);
                            }
                        }
                    }
                }
            }
        }

        if response.double_clicked_by(Primary)
            && let Some(pos) = response.interact_pointer_pos()
            && let Some((i, subs)) = &edited
            && hit(self, pos).is_none()
            && let Some((s, k, t, d)) = nodos::segmento_cercano(subs, vista.to_mm(pos))
            && d <= f64::from(PICK_PX / (10.0 * vista.scale))
        {
            let mut nuevo = None;
            self.proyecto.as_mut().unwrap().edit_nodes(*i, |subs| {
                nuevo = Some(nodos::insertar(subs, s, k, t));
                true
            });
            self.nodos_sel = nuevo.into_iter().collect();
        } else if response.clicked_by(Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            match hit(self, pos) {
                Some(NodeHit::Nodo(id)) => {
                    let ya = self.nodos_sel.contains(&id);
                    if sumar && ya {
                        self.nodos_sel.retain(|&x| x != id);
                    } else if !sumar {
                        self.nodos_sel = vec![id];
                    } else {
                        self.nodos_sel.push(id);
                    }
                }
                Some(NodeHit::Manija(..)) => {}
                None => {
                    // Otro objeto: pasa a editarse; el vacío suelta los nodos y luego el objeto.
                    let doc = self.proyecto.as_mut().unwrap();
                    let tol = f64::from(PICK_PX / (10.0 * vista.scale));
                    match doc.object_at(vista.to_mm(pos), tol) {
                        Some(o) if Some(o) != self.nodos_obj => {
                            doc.seleccion.fill(false);
                            doc.seleccion[o] = true;
                        }
                        Some(_) => {}
                        None if self.nodos_sel.is_empty() && !sumar => doc.seleccion.fill(false),
                        None => {
                            if !sumar {
                                self.nodos_sel.clear();
                            }
                        }
                    }
                }
            }
        }
    }

    fn draw_nodes(&self, painter: &egui::Painter, vista: Vista) {
        let Some(subs) = self.proyecto.as_ref().and_then(|d| d.nodes(d.editable()?)) else {
            return;
        };
        let stroke = Stroke::new(1.5, SELECTION_COLOR);
        let thin = Stroke::new(1.0, SELECTION_COLOR);
        for &id in &self.nodos_sel {
            let Some(n) = nodos::nodo(&subs, id) else {
                continue;
            };
            for q in [n.entrada, n.salida] {
                if q.distance(n.p) > 1e-6 {
                    painter.line_segment([vista.mm(n.p), vista.mm(q)], thin);
                    painter.circle(vista.mm(q), 3.5, Color32::WHITE, stroke);
                }
            }
        }
        for id in nodos::todos(&subs) {
            let n = nodos::nodo(&subs, id).unwrap();
            let p = vista.mm(n.p);
            let fill = if self.nodos_sel.contains(&id) {
                SELECTION_COLOR
            } else {
                Color32::WHITE
            };
            if n.es_esquina() {
                let q = Rect::from_center_size(p, Vec2::splat(7.0));
                painter.rect(q, 0.0, fill, stroke, egui::StrokeKind::Middle);
            } else {
                painter.circle(p, 4.0, fill, stroke);
            }
        }
    }

    /// Herramienta Texto: clic en un texto lo sigue editando; en otro lugar crea uno nuevo
    /// con la línea base ahí. Mientras se escribe, clic afuera termina.
    fn text_input(&mut self, ui: &egui::Ui, response: &Response, vista: Vista) {
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if !response.clicked_by(egui::PointerButton::Primary) {
            return;
        }
        let Some(pos) = response.interact_pointer_pos() else {
            return;
        };
        let p = vista.to_mm(pos);
        let tol = f64::from(PICK_PX / (10.0 * vista.scale));
        let doc = self.proyecto.as_ref().unwrap();
        let texto = doc
            .object_at(p, tol)
            .filter(|&i| matches!(doc.proyecto.objetos[i].forma, bordado::Forma::Texto { .. }));
        match (texto, self.writing()) {
            (Some(i), Some(j)) if i == j => {}
            (Some(i), _) => self.edit_text(i),
            // Escribiendo, el clic afuera solo termina.
            (None, Some(_)) => self.finish_text(),
            (None, None) => self.new_text_at(p),
        }
    }

    /// Cursor del texto que se escribe.
    fn draw_text_cursor(&self, ui: &egui::Ui, painter: &egui::Painter, vista: Vista) {
        let Some((i, antes)) = self.writing_cursor() else {
            return;
        };
        let doc = self.proyecto.as_ref().unwrap();
        let bordado::Forma::Texto {
            fuente,
            alto,
            espaciado,
            afin,
            ..
        } = &doc.proyecto.objetos[i].forma
        else {
            return;
        };
        let Ok(fin) = bordado::texto::fin(&antes, fuente, *alto, *espaciado) else {
            return;
        };
        let af = kurbo::Affine::new(*afin);
        let arriba = vista.mm(af * (fin + kurbo::Vec2::new(0.0, -alto * 1.05)));
        let abajo = vista.mm(af * (fin + kurbo::Vec2::new(0.0, alto * 0.25)));
        // Parpadea como un cursor de texto.
        let t = ui.input(|i| i.time);
        if t.fract() < 0.6 {
            painter.line_segment([arriba, abajo], Stroke::new(2.0, crate::tema::COPIHUE));
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }

    /// Rectángulo de selección y medidas del gesto junto al puntero.
    fn draw_gesture(&self, painter: &egui::Painter, response: &Response, vista: Vista) {
        let Some(g) = self.gesto else {
            return;
        };
        let text = match g {
            Gesto::Caja { a, b, .. } => {
                draw_box(painter, Rect::from_two_pos(vista.mm(a), vista.mm(b)));
                return;
            }
            Gesto::Angulo { angulo, .. } => format!("Filas a {angulo:.0}°"),
            Gesto::Transformar { tipo, r0, af, .. } => match tipo {
                Tipo::Mover => {
                    let d = af.translation();
                    format!("Δx {:.1}  Δy {:.1} mm", d.x, d.y)
                }
                Tipo::Escalar(..) => {
                    let r = af.transform_rect_bbox(r0);
                    let pct = if r0.width() > 1e-9 {
                        r.width() / r0.width()
                    } else {
                        r.height() / r0.height().max(1e-9)
                    } * 100.0;
                    format!("{:.1} × {:.1} mm  ({pct:.0} %)", r.width(), r.height())
                }
                Tipo::Girar => {
                    let [a, b, ..] = af.as_coeffs();
                    format!("{:.1}°", b.atan2(a).to_degrees())
                }
            },
        };
        if let Some(pos) = response.hover_pos().or(response.interact_pointer_pos()) {
            let at = pos + Vec2::new(14.0, 14.0);
            let galley = painter.layout_no_wrap(text, FontId::proportional(13.0), Color32::WHITE);
            let bg = Rect::from_min_size(at, galley.size()).expand(3.0);
            painter.rect_filled(bg, 3.0, Color32::from_black_alpha(170));
            painter.galley(at, galley, Color32::WHITE);
        }
    }
}

/// Arrastre con la herramienta de nodos.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum GestoNodo {
    /// Mueve los nodos seleccionados; `ultimo` = posición anterior del puntero (mm).
    Mover {
        ultimo: kurbo::Point,
    },
    Manija {
        id: nodos::Id,
        lado: Lado,
    },
    Caja {
        a: kurbo::Point,
        b: kurbo::Point,
        sumar: bool,
    },
}

#[derive(Clone, Copy, PartialEq)]
enum NodeHit {
    Nodo(nodos::Id),
    Manija(nodos::Id, Lado),
}

/// Lo que hay bajo el puntero con la herramienta de selección.
#[derive(Clone, Copy, PartialEq)]
enum Pick {
    Manija(Tipo),
    /// La manija del ángulo del relleno de un objeto.
    Angulo(usize),
    /// Un objeto seleccionado o el interior del marco de la selección.
    Seleccion,
    Objeto(usize),
    Fondo,
    Nada,
}

/// Manijas de un marco en pantalla: esquinas y, con `completo`, lados y giro.
fn handles(r: Rect, completo: bool) -> Vec<(Tipo, Pos2)> {
    let mut v = vec![
        (Tipo::Escalar(-1, -1), r.left_top()),
        (Tipo::Escalar(1, -1), r.right_top()),
        (Tipo::Escalar(1, 1), r.right_bottom()),
        (Tipo::Escalar(-1, 1), r.left_bottom()),
    ];
    if completo {
        v.extend([
            (Tipo::Escalar(0, -1), r.center_top()),
            (Tipo::Escalar(1, 0), r.right_center()),
            (Tipo::Escalar(0, 1), r.center_bottom()),
            (Tipo::Escalar(-1, 0), r.left_center()),
            (Tipo::Girar, r.center_top() - Vec2::new(0.0, ROTATE_PX)),
        ]);
    }
    v
}

fn handle_cursor(t: Tipo) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match t {
        Tipo::Escalar(0, _) => C::ResizeVertical,
        Tipo::Escalar(_, 0) => C::ResizeHorizontal,
        Tipo::Escalar(x, y) if x == y => C::ResizeNwSe,
        Tipo::Escalar(..) => C::ResizeNeSw,
        Tipo::Girar => C::Alias,
        Tipo::Mover => C::Move,
    }
}

/// Manija del ángulo del relleno: una línea por el centro en la dirección de las filas.
fn draw_angle_handle(painter: &egui::Painter, centro: Pos2, punta: Pos2) {
    let atras = centro - (punta - centro);
    let halo = Stroke::new(3.5, Color32::from_white_alpha(200));
    let linea = Stroke::new(1.5, ANGLE_COLOR);
    painter.line_segment([atras, punta], halo);
    painter.line_segment([atras, punta], linea);
    painter.circle_filled(centro, 2.5, ANGLE_COLOR);
    painter.circle(punta, 5.0, Color32::WHITE, Stroke::new(1.5, ANGLE_COLOR));
}

fn draw_box(painter: &egui::Painter, r: Rect) {
    painter.rect_filled(r, 0.0, SELECTION_COLOR.gamma_multiply(0.12));
    painter.rect_stroke(
        r,
        0.0,
        Stroke::new(1.0, SELECTION_COLOR),
        egui::StrokeKind::Middle,
    );
}

fn draw_handles(painter: &egui::Painter, r: Rect, completo: bool) {
    draw_selection_frame(painter, r);
    let stroke = Stroke::new(1.5, SELECTION_COLOR);
    if completo {
        painter.line_segment(
            [r.center_top(), r.center_top() - Vec2::new(0.0, ROTATE_PX)],
            stroke,
        );
    }
    for (t, p) in handles(r, completo) {
        if t == Tipo::Girar {
            painter.circle(p, 4.5, Color32::WHITE, stroke);
        } else {
            let q = Rect::from_center_size(p, Vec2::splat(7.0));
            painter.rect(q, 0.0, Color32::WHITE, stroke, egui::StrokeKind::Middle);
        }
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

/// Dibuja las primeras `progress` entradas de los bloques visibles. `moved`: entradas que se
/// dibujan con una transformación (mm), la vista previa de un gesto.
#[allow(clippy::too_many_arguments)]
fn draw_stitches(
    painter: &egui::Painter,
    vista: Vista,
    jef: &Jef,
    blocks: &[Range<usize>],
    visible: impl Fn(usize) -> bool,
    progress: usize,
    show_jumps: bool,
    moved: Option<(&[bool], kurbo::Affine)>,
    realista: Option<&realista::Texturas>,
) {
    let pt = |i: usize| {
        let s = jef.stitches[i];
        match moved {
            Some((m, af)) if m[i] => {
                vista.mm(af * kurbo::Point::new(f64::from(s.x) / 10.0, f64::from(s.y) / 10.0))
            }
            _ => vista.p(s.x, s.y),
        }
    };
    // Vista realista solo si el hilo se ve con algo de grosor.
    let hilo = realista::ancho_px(vista.scale * 10.0);
    let realista = realista.filter(|_| hilo >= realista::MINIMO_PX);
    let mut malla = realista.map(|t| t.malla());
    let mut bloque = realista.map(|t| t.bloque());
    let recorte = painter.clip_rect();
    // Grosor aproximado de un hilo (0,4 mm), con un mínimo para que se vea al alejar.
    let width = (4.0 * vista.scale).clamp(0.6, 6.0);
    let jump_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(40, 40, 40, 160));
    let mut shapes = Vec::new();
    for (b, block) in blocks.iter().enumerate() {
        if !visible(b) || block.start >= progress {
            continue;
        }
        let [r, g, bl] = jef.thread_rgb(b);
        let color = Color32::from_rgb(r, g, bl);
        let stroke = Stroke::new(width, color);
        let end = block.end.min(progress);
        let mut run: Vec<Pos2> = Vec::new();
        for i in block.start..end {
            let s = jef.stitches[i];
            let p = pt(i);
            if s.kind == StitchKind::Normal {
                let antes = i.checked_sub(1).map_or(vista.p(0, 0), pt);
                if let Some(bl) = &mut bloque {
                    bl.puntada(antes, p, hilo, color, recorte);
                    continue;
                }
                if run.is_empty() {
                    run.push(antes);
                }
                push_point(&mut shapes, &mut run, p, stroke);
            } else {
                flush_run(&mut shapes, &mut run, stroke);
                if show_jumps && s.kind == StitchKind::Jump && i > 0 {
                    shapes.extend(Shape::dashed_line(&[pt(i - 1), p], jump_stroke, 4.0, 3.0));
                }
            }
        }
        flush_run(&mut shapes, &mut run, stroke);
        if let (Some(bl), Some(m)) = (&mut bloque, &mut malla) {
            bl.cerrar(m);
        }
    }
    if let Some(m) = malla {
        painter.add(Shape::mesh(m));
    }
    painter.extend(shapes);
}

/// Agrega `p` a la polilínea en curso. egui une los tramos en inglete sin límite: en vueltas
/// cerradas (remates, ida y vuelta) eso dibuja púas largas, y los tramos de largo 0 dan
/// esquinas erráticas. Por eso se omiten los puntos repetidos y la línea se corta en cada vuelta
/// de más de ~110°, tapando el corte con un círculo del grosor del hilo.
fn push_point(shapes: &mut Vec<Shape>, run: &mut Vec<Pos2>, p: Pos2, stroke: Stroke) {
    let Some(&last) = run.last() else {
        run.push(p);
        return;
    };
    if (p - last).length_sq() < 0.25 {
        return;
    }
    if let [.., before, _] = run[..] {
        let (a, b) = ((last - before).normalized(), (p - last).normalized());
        if a.dot(b) < -0.35 {
            flush_run(shapes, run, stroke);
            shapes.push(Shape::circle_filled(last, stroke.width / 2.0, stroke.color));
            run.push(last);
        }
    }
    run.push(p);
}

fn flush_run(shapes: &mut Vec<Shape>, run: &mut Vec<Pos2>, stroke: Stroke) {
    if run.len() > 1 {
        shapes.push(Shape::line(std::mem::take(run), stroke));
    }
    run.clear();
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
