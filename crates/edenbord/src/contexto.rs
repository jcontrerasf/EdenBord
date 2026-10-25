//! Barra contextual, bajo la barra superior: cambia según la herramienta y la selección
//! (posición y tamaño, girar, espejar, escalar, trazados, nodos, pluma, regla).

use bordado::booleanas::Booleana;
use eframe::egui::{self, RichText, Ui};
use kurbo::{Affine, Vec2};

use crate::{Editor, Op, Tool, apply_jef, apply_proyecto, iconos, tema};

impl Editor {
    pub(crate) fn context_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            match self.tool {
                Tool::Select => self.ctx_select(ui),
                Tool::Nodes => self.ctx_nodes(ui),
                Tool::Pen => self.ctx_pen(ui),
                Tool::Ruler => self.ctx_ruler(ui),
                Tool::Texto => self.ctx_text(ui),
            }
        });
    }

    fn ctx_text(&mut self, ui: &mut Ui) {
        let ayuda = if self.writing().is_some() {
            "Escribiendo · Enter: otra línea · Esc o clic afuera: terminar"
        } else {
            "Clic en el lienzo para escribir un texto · clic en un texto para seguirlo"
        };
        ui.label(RichText::new(ayuda).color(tema::TEXTO_SUAVE));
        let fuente = self
            .fuente
            .clone()
            .or_else(bordado::fuentes::predeterminada)
            .map_or_else(|| "(sin fuentes)".to_string(), |f| f.nombre());
        ui.separator();
        ui.label(RichText::new(format!("Fuente: {fuente}")).color(tema::TEXTO_SUAVE))
            .on_hover_text("Se cambia en Propiedades, con el texto elegido");
    }

    fn ctx_select(&mut self, ui: &mut Ui) {
        let n = match (&self.doc, &self.proyecto) {
            (Some(d), _) => d.selection().len(),
            (_, Some(d)) => d.selection().len(),
            _ => return,
        };
        let titulo = match n {
            0 => "Todo".to_string(),
            1 => "Selección".to_string(),
            n => format!("Selección · {n}"),
        };
        ui.label(RichText::new(titulo).color(tema::TEXTO_SUAVE));
        if self.proyecto.is_some() {
            self.geometry_proyecto(ui);
        } else {
            self.geometry_jef(ui);
        }
        ui.separator();
        if let Some(op) = self.transform_buttons(ui) {
            if let Some(doc) = &mut self.doc {
                apply_jef(doc, op);
            }
            if let Some(doc) = &mut self.proyecto {
                apply_proyecto(doc, op);
            }
        }
        if self.proyecto.is_some() {
            self.path_buttons(ui);
        }
    }

    /// X, Y, ancho y alto de la selección (o de todo) en mm, editables.
    fn geometry_proyecto(&mut self, ui: &mut Ui) {
        let lock = &mut self.lock_ratio;
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let sel = doc.selection();
        let Some(r) = doc.rect_of(&sel) else {
            return;
        };
        let (mut x, mut y, mut w, mut h) = (r.x0, r.y0, r.width(), r.height());
        let cx = campo(ui, "X", &mut x, true);
        let cy = campo(ui, "Y", &mut y, true);
        let cw = campo(ui, "An", &mut w, r.width() > 0.01);
        let (ico, ayuda) = if *lock {
            (iconos::PROPORCIONAL, "Proporcional: ancho y alto juntos")
        } else {
            (iconos::LIBRE, "Ancho y alto por separado")
        };
        if tema::interruptor(ui, *lock, ico, ayuda).clicked() {
            *lock = !*lock;
        }
        let ch = campo(ui, "Al", &mut h, r.height() > 0.01);
        let p0 = Vec2::new(r.x0, r.y0);
        let about = |t: Affine| Affine::translate(p0) * t * Affine::translate(-p0);
        let t = if cx || cy {
            Some(("mover", Affine::translate((x - r.x0, y - r.y0))))
        } else if cw && w > 0.01 {
            let s = w / r.width();
            let sy = if *lock { s } else { 1.0 };
            Some(("tamaño", about(Affine::scale_non_uniform(s, sy))))
        } else if ch && h > 0.01 {
            let s = h / r.height();
            let sx = if *lock { s } else { 1.0 };
            Some(("tamaño", about(Affine::scale_non_uniform(sx, s))))
        } else {
            None
        };
        if let Some((key, t)) = t {
            doc.transform_grouped(key, &sel, t);
        }
    }

    /// En un JEF: X e Y editables (mueven en 0,1 mm); el tamaño solo se muestra (escalar
    /// puntadas a mano pierde precisión: para eso está el %).
    fn geometry_jef(&mut self, ui: &mut Ui) {
        let Some(doc) = &mut self.doc else {
            return;
        };
        let sel = doc.selection();
        let Some(b) = doc.jef.selection_bounds(&sel) else {
            return;
        };
        let (x0, y0) = (f64::from(b.min_x) / 10.0, f64::from(b.min_y) / 10.0);
        let (mut x, mut y) = (x0, y0);
        let cx = campo(ui, "X", &mut x, true);
        let cy = campo(ui, "Y", &mut y, true);
        if cx || cy {
            let dx = ((x - x0) * 10.0).round() as i32;
            let dy = ((y - y0) * 10.0).round() as i32;
            if dx != 0 || dy != 0 {
                doc.edit_grouped("mover", |j| j.translate(&sel, dx, dy));
            }
        }
        let (w, h) = (
            f64::from(b.max_x - b.min_x) / 10.0,
            f64::from(b.max_y - b.min_y) / 10.0,
        );
        ui.label(RichText::new(format!("{w:.1} × {h:.1} mm")).color(tema::TEXTO_SUAVE));
    }

    /// Girar, espejar, escalar y centrar; paso de las flechas del teclado.
    fn transform_buttons(&mut self, ui: &mut Ui) -> Option<Op> {
        let mut op = None;
        if tema::boton_icono(ui, iconos::GIRAR_IZQ, "Girar 90° a la izquierda").clicked() {
            op = Some(Op::Rotate(-90.0));
        }
        if tema::boton_icono(ui, iconos::GIRAR_DER, "Girar 90° a la derecha").clicked() {
            op = Some(Op::Rotate(90.0));
        }
        ui.add(
            egui::DragValue::new(&mut self.angle)
                .range(-180.0..=180.0)
                .speed(1.0)
                .suffix("°"),
        )
        .on_hover_text("Ángulo para girar");
        if tema::boton_icono(ui, iconos::GIRAR, "Girar ese ángulo").clicked() {
            op = Some(Op::Rotate(self.angle));
        }
        if tema::boton_icono(ui, iconos::ESPEJO_H, "Espejo horizontal").clicked() {
            op = Some(Op::MirrorX);
        }
        if tema::boton_icono(ui, iconos::ESPEJO_V, "Espejo vertical").clicked() {
            op = Some(Op::MirrorY);
        }
        ui.separator();
        // En un JEF escalar no regenera la densidad: avisar fuera de lo seguro.
        let riesgo = self.doc.is_some() && !jef::SAFE_SCALE.contains(&(self.scale_pct / 100.0));
        let pct = ui.add(
            egui::DragValue::new(&mut self.scale_pct)
                .range(10.0..=500.0)
                .speed(1.0)
                .suffix(" %"),
        );
        if riesgo {
            pct.on_hover_text(format!(
                "{} Fuera de 80–120 % la densidad del relleno cambia mucho",
                iconos::AVISO
            ));
            ui.label(RichText::new(iconos::AVISO).color(tema::AVISO));
        }
        if tema::boton_icono(ui, iconos::ESCALAR, "Escalar ese porcentaje").clicked() {
            op = Some(Op::Scale(self.scale_pct / 100.0));
        }
        if tema::boton_icono(ui, iconos::CENTRAR, "Centrar en el bastidor").clicked() {
            op = Some(Op::Center);
        }
        ui.separator();
        ui.label(RichText::new(iconos::PASO).color(tema::TEXTO_SUAVE))
            .on_hover_text("Paso de las flechas del teclado");
        ui.add(
            egui::DragValue::new(&mut self.step_mm)
                .range(0.1..=50.0)
                .speed(0.1)
                .suffix(" mm"),
        )
        .on_hover_text("Paso de las flechas del teclado");
        op
    }

    /// Combinar, separar y booleanas, si la selección lo permite.
    fn path_buttons(&mut self, ui: &mut Ui) {
        let doc = self.proyecto.as_mut().unwrap();
        let sel = doc.selection();
        let varias = sel.len() > 1;
        let separable = sel.iter().any(|&i| doc.subpaths(i) > 1);
        if !varias && !separable {
            return;
        }
        ui.separator();
        if varias
            && tema::boton_icono(
                ui,
                iconos::COMBINAR,
                "Combinar zonas: une los contornos; los interiores quedan como agujeros",
            )
            .clicked()
            && !doc.combine()
        {
            self.error("Selecciona al menos dos contornos para combinar");
            return;
        }
        let doc = self.proyecto.as_mut().unwrap();
        if separable
            && tema::boton_icono(
                ui,
                iconos::SEPARAR,
                "Separar trazados: cada parte pasa a ser un objeto; los agujeros quedan como \
                 zonas aparte (bórralos o vuelve a combinarlos). Los textos: primero «Convertir \
                 en trazado»",
            )
            .clicked()
        {
            doc.split();
        }
        if !varias {
            return;
        }
        let mut op = None;
        for b in Booleana::TODAS {
            let (ico, ayuda) = match b {
                Booleana::Unir => (iconos::UNIR, "Unir: las zonas en una sola"),
                Booleana::Restar => (
                    iconos::RESTAR,
                    "Restar: recorta la primera (la que se borda antes) con las demás, que se \
                     conservan: el fondo no se cose bajo las piezas de encima",
                ),
                Booleana::Intersecar => (
                    iconos::INTERSECAR,
                    "Intersecar: solo lo que tienen en común",
                ),
                Booleana::Excluir => (iconos::EXCLUIR, "Excluir: lo que cubre una sola zona"),
            };
            if tema::boton_icono(ui, ico, ayuda).clicked() {
                op = Some(b);
            }
        }
        let doc = self.proyecto.as_mut().unwrap();
        if let Some(op) = op
            && let Err(e) = doc.boolean(op)
        {
            self.error(e);
        }
    }

    fn ctx_nodes(&mut self, ui: &mut Ui) {
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let Some(i) = doc.editable() else {
            ayuda(ui, "Elige un contorno o una línea para editar sus nodos.");
            return;
        };
        let ids = self.nodos_sel.clone();
        let hay = !ids.is_empty();
        if ui
            .add_enabled(
                hay,
                egui::Button::new(format!("{} Esquina ↔ curva", iconos::ESQUINA_CURVA)),
            )
            .clicked()
        {
            doc.edit_nodes(i, |subs| {
                crate::nodos::alternar_curva(subs, &ids);
                true
            });
        }
        if ui
            .add_enabled(
                hay,
                egui::Button::new(format!("{} Borrar nodos", iconos::BORRAR)),
            )
            .on_hover_text("Supr")
            .clicked()
        {
            doc.edit_nodes(i, |subs| crate::nodos::borrar(subs, &ids));
            self.nodos_sel.clear();
        }
        ui.separator();
        ayuda(
            ui,
            "Arrastrar: mover nodos o manijas · Shift/Ctrl+clic: agregar · arrastrar en el \
             vacío: elegir varios · doble clic en el trazo: nuevo nodo",
        );
    }

    fn ctx_pen(&mut self, ui: &mut Ui) {
        if ui
            .add_enabled(
                self.pen.cerrable(),
                egui::Button::new(format!("{} Cerrar zona", iconos::CERRAR)),
            )
            .on_hover_text("O clic en el primer vértice")
            .clicked()
        {
            self.finish_pen(true);
        }
        if ui
            .add_enabled(
                self.pen.nodos.len() >= 2,
                egui::Button::new(format!("{} Terminar línea", iconos::LISTO)),
            )
            .on_hover_text("Enter")
            .clicked()
        {
            self.finish_pen(false);
        }
        if ui
            .add_enabled(
                !self.pen.nodos.is_empty(),
                egui::Button::new(format!("{} Cancelar", iconos::CANCELAR)),
            )
            .on_hover_text("Esc")
            .clicked()
        {
            self.pen = Default::default();
        }
        ui.separator();
        ayuda(
            ui,
            "Clic: esquina · arrastrar: curva · Retroceso o Ctrl+Z: borrar vértice · Espacio + \
             arrastrar: mover la vista",
        );
    }

    fn ctx_ruler(&mut self, ui: &mut Ui) {
        let Some((a, b)) = self.ruler else {
            ayuda(
                ui,
                "Arrastra en el lienzo para medir · Shift: múltiplos de 45°",
            );
            return;
        };
        let d = b - a;
        // y crece hacia abajo en el modelo: el ángulo se muestra como en un plano normal.
        let angulo = (-d.y).atan2(d.x).to_degrees();
        ui.label(
            RichText::new(format!(
                "{:.1} mm   Δx {:.1}   Δy {:.1}   {angulo:.1}°",
                d.hypot(),
                d.x,
                d.y
            ))
            .monospace(),
        );
        if ui
            .button(format!("{} Borrar", iconos::CANCELAR))
            .on_hover_text("Esc")
            .clicked()
        {
            self.ruler = None;
        }
    }
}

/// Campo en mm con su rótulo. Devuelve si cambió.
fn campo(ui: &mut Ui, rotulo: &str, v: &mut f64, enabled: bool) -> bool {
    ui.label(RichText::new(rotulo).color(tema::TEXTO_SUAVE));
    ui.add_enabled(
        enabled,
        egui::DragValue::new(v)
            .speed(0.1)
            .max_decimals(1)
            .suffix(" mm"),
    )
    .changed()
}

fn ayuda(ui: &mut Ui, texto: &str) {
    ui.label(RichText::new(texto).small().color(tema::TEXTO_SUAVE));
}
