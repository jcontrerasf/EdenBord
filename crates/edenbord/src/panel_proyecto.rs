//! Panel lateral de un proyecto `.edenbord`: objetos, tipo de puntada, guardar y exportar.

use std::path::PathBuf;

use bordado::{Fondo, Forma, Objeto, PRESETS, Proyecto, Puntada};
use eframe::egui::{self, Color32, Vec2};
use jef::{Hoop, StitchKind};
use kurbo::{Affine, Point, Shape};

use crate::{Editor, Tool, file_name, write_verified};

const WARNING: Color32 = Color32::from_rgb(200, 120, 0);

impl Editor {
    pub(crate) fn proyecto_panel(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let mut name = doc.path.as_deref().map_or(doc.nombre.clone(), file_name);
        if doc.dirty {
            name.push_str(" •");
        }
        ui.heading(name);
        let mut save = None;
        let mut export = false;
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("💾 Guardar")
                .on_hover_text("Ctrl+S · proyecto editable .edenbord")
                .clicked()
            {
                save = Some(false);
            }
            if ui.button("Guardar como…").clicked() {
                save = Some(true);
            }
            if ui
                .button("📤 Exportar JEF…")
                .on_hover_text("Genera las puntadas para la máquina")
                .clicked()
            {
                export = true;
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(doc.can_undo(), egui::Button::new("↶ Deshacer"))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                doc.undo();
            }
            if ui
                .add_enabled(doc.can_redo(), egui::Button::new("↷ Rehacer"))
                .on_hover_text("Ctrl+Y")
                .clicked()
            {
                doc.redo();
            }
        });
        let mut import = false;
        let mut text = false;
        ui.horizontal(|ui| {
            import = ui.button("➕ Importar SVG…").clicked();
            text = ui.button("🔤 Agregar texto…").clicked();
        });
        if let Some(ask) = save {
            self.save_project(ask);
        }
        if export {
            self.export_jef();
        }
        if import {
            self.import_svg();
        }
        if text {
            self.add_text();
        }
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let doc = self.proyecto.as_mut().unwrap();
            info_section(ui, doc);
            ui.separator();
            self.trace_section(ui);
            ui.separator();
            let doc = self.proyecto.as_mut().unwrap();
            if let Some(i) = objects_section(ui, doc) {
                self.picking_color = Some(i);
            }
            ui.separator();
            let doc = self.proyecto.as_mut().unwrap();
            if let Some(path) = properties_section(ui, doc) {
                self.change_font(path);
            }
            ui.separator();
            let doc = self.proyecto.as_ref().unwrap();
            let label = if doc.selection().is_empty() {
                "Transformar (todo el proyecto)"
            } else {
                "Transformar (selección)"
            };
            if let Some(op) = self.transform_section(ui, label, false) {
                crate::apply_proyecto(self.proyecto.as_mut().unwrap(), op);
            }
            ui.separator();
            self.view_sections(ui);
        });
    }

    /// Sección "Calcar": imagen de fondo y herramienta pluma.
    fn trace_section(&mut self, ui: &mut egui::Ui) {
        ui.strong("Calcar");
        if self.tool != Tool::Pen {
            ui.small("Elige ✒ Pluma en la barra del lienzo para dibujar contornos.");
        } else {
            ui.small(
                "Clic: vértice en esquina · arrastrar: vértice con curva · clic en el primero: \
                 cerrar zona · Enter: terminar línea · Retroceso: borrar vértice · Esc: cancelar \
                 · rueda o botón central: mover la vista",
            );
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.pen.cerrable(), egui::Button::new("Cerrar zona"))
                    .clicked()
                {
                    self.finish_pen(true);
                }
                if ui
                    .add_enabled(
                        self.pen.nodos.len() >= 2,
                        egui::Button::new("Terminar línea"),
                    )
                    .clicked()
                {
                    self.finish_pen(false);
                }
                if ui
                    .add_enabled(!self.pen.nodos.is_empty(), egui::Button::new("Cancelar"))
                    .clicked()
                {
                    self.pen = Default::default();
                }
            });
        }
        let doc = self.proyecto.as_mut().unwrap();
        if doc.selection().len() > 1
            && ui
                .button("⧉ Combinar zonas")
                .on_hover_text(
                    "Une los contornos seleccionados; los interiores quedan como agujeros",
                )
                .clicked()
            && !doc.combine()
        {
            self.error("Selecciona al menos dos contornos para combinar");
        }

        let doc = self.proyecto.as_mut().unwrap();
        let mut pick = false;
        ui.horizontal(|ui| {
            ui.label("Imagen de fondo");
            pick = ui.button("Elegir…").clicked();
            if doc.proyecto.fondo.is_some() && ui.button("Quitar").clicked() {
                doc.edit(|p| p.fondo = None);
            }
        });
        if let Some(fondo) = &doc.proyecto.fondo {
            let mut f = fondo.clone();
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.visible, "Ver");
                ui.add(egui::Slider::new(&mut f.opacidad, 0.05..=1.0).text("opacidad"));
            });
            ui.horizontal(|ui| {
                ui.label("Ancho");
                ui.add(
                    egui::DragValue::new(&mut f.ancho)
                        .range(5.0..=500.0)
                        .speed(0.2)
                        .suffix(" mm"),
                );
                ui.label("Centro");
                ui.add(
                    egui::DragValue::new(&mut f.centro[0])
                        .speed(0.2)
                        .suffix(" mm"),
                );
                ui.add(
                    egui::DragValue::new(&mut f.centro[1])
                        .speed(0.2)
                        .suffix(" mm"),
                );
            });
            ui.small("El fondo no se borda ni se exporta.");
            if &f != fondo {
                doc.edit_grouped("fondo".into(), |p| p.fondo = Some(f));
            }
        }
        if pick {
            self.pick_fondo();
        }
    }

    fn pick_fondo(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Imagen",
                &["png", "jpg", "jpeg", "webp", "bmp", "PNG", "JPG", "JPEG"],
            )
            .pick_file()
        else {
            return;
        };
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        // Ancho inicial: el del bastidor, para empezar con algo del tamaño del diseño.
        let ancho = jef::Hoop::from_code(doc.proyecto.bastidor)
            .size()
            .map_or(100.0, |(w, _)| f64::from(w) / 10.0);
        doc.edit(|p| {
            p.fondo = Some(Fondo {
                ruta: path,
                centro: [0.0, 0.0],
                ancho,
                opacidad: 0.6,
                visible: true,
            });
        });
        self.fit_pending = true;
    }

    /// Carga la textura del fondo si cambió la imagen.
    pub(crate) fn load_fondo(&mut self, ctx: &egui::Context) {
        let path = self.proyecto.as_ref().and_then(|d| d.fondo_path());
        let Some(path) = path else {
            self.fondo_tex = None;
            return;
        };
        if self.fondo_tex.as_ref().is_some_and(|(p, _)| *p == path) {
            return;
        }
        let tex = image::open(&path).map(|img| {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            ctx.load_texture("fondo", color, egui::TextureOptions::LINEAR)
        });
        match tex {
            Ok(t) => self.fondo_tex = Some((path, Some(t))),
            Err(e) => {
                self.error(format!("No se pudo leer la imagen {}: {e}", path.display()));
                self.fondo_tex = Some((path, None));
            }
        }
    }

    /// Convierte el trazo de la pluma en un objeto guía (zona si `cerrar`, si no línea).
    pub(crate) fn finish_pen(&mut self, cerrar: bool) {
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let cerrar = cerrar && self.pen.cerrable();
        if !cerrar && self.pen.nodos.len() < 2 {
            return;
        }
        let d = self.pen.trazado(cerrar);
        let n = doc.proyecto.objetos.len() + 1;
        let (nombre, forma) = if cerrar {
            (format!("zona {n}"), Forma::Contorno { d })
        } else {
            (format!("línea {n}"), Forma::Linea { d })
        };
        doc.add(vec![Objeto {
            nombre,
            visible: true,
            color: 1,
            forma,
            puntada: Puntada::Ninguna,
        }]);
        self.pen = Default::default();
    }

    /// Guarda el `.edenbord`; con `ask` (o si nunca se guardó) pregunta dónde.
    pub(crate) fn save_project(&mut self, ask: bool) {
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        // Un `.bordado` antiguo se guarda como `.edenbord` nuevo (pregunta dónde).
        let path = match (&doc.path, ask) {
            (Some(p), false) if p.extension().is_some_and(|e| e == bordado::EXTENSION) => p.clone(),
            _ => {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("Proyecto EdenBord", &[bordado::EXTENSION])
                    .set_file_name(format!("{}.{}", doc.nombre, bordado::EXTENSION));
                if let Some(dir) = doc.path.as_deref().and_then(|p| p.parent()) {
                    dialog = dialog.set_directory(dir);
                }
                let Some(mut p) = dialog.save_file() else {
                    return;
                };
                if p.extension().is_none() {
                    p.set_extension(bordado::EXTENSION);
                }
                p
            }
        };
        let text = doc.proyecto.guardar();
        let result = std::fs::write(&path, &text)
            .and_then(|()| std::fs::read_to_string(&path))
            .map_err(|e| e.to_string())
            .and_then(|back| {
                let again = Proyecto::leer(&back).map_err(|e| e.to_string())?;
                if again.guardar() == text {
                    Ok(())
                } else {
                    Err("el archivo releído no coincide".to_string())
                }
            });
        match result {
            Ok(()) => {
                doc.nombre = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                doc.path = Some(path.clone());
                doc.dirty = false;
                self.info(format!("Proyecto guardado: {}", file_name(&path)));
            }
            Err(e) => self.error(format!("Error al guardar {}: {e}", path.display())),
        }
    }

    fn export_jef(&mut self) {
        let Some(doc) = &self.proyecto else {
            return;
        };
        let mut dialog = crate::jef_dialog().set_file_name(format!("{}.jef", doc.nombre));
        if let Some(dir) = doc.path.as_deref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        let avisos = doc.generado.avisos.len();
        match write_verified(&path, &doc.generado.jef) {
            Ok(()) if avisos > 0 => self.error(format!(
                "Exportado {}, pero {avisos} objetos no se pudieron bordar (ver avisos)",
                file_name(&path)
            )),
            Ok(()) => self.info(format!("Exportado y verificado: {}", file_name(&path))),
            Err(e) => self.error(format!("Error al exportar {}: {e}", path.display())),
        }
    }

    fn import_svg(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Vector SVG", &["svg", "SVG"])
            .pick_file()
        else {
            return;
        };
        match bordado::svg::importar_archivo(&path) {
            Ok(objetos) => {
                let n = objetos.len();
                if let Some(doc) = &mut self.proyecto {
                    doc.add(objetos);
                }
                self.info(format!(
                    "{n} trazados de {} agregados como guías y seleccionados",
                    file_name(&path)
                ));
            }
            Err(e) => self.error(format!("No se pudo importar {}: {e}", path.display())),
        }
    }

    fn pick_font(&mut self) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new().add_filter("Fuente", &["ttf", "otf", "TTF", "OTF"]);
        if let Some(dir) = self.last_font.as_deref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(dir);
        }
        let path = dialog.pick_file()?;
        self.last_font = Some(path.clone());
        Some(path)
    }

    /// Agrega un texto centrado en el bastidor, con relleno.
    fn add_text(&mut self) {
        let Some(fuente) = self.last_font.clone().or_else(|| self.pick_font()) else {
            return;
        };
        let mut obj = Objeto {
            nombre: "texto".into(),
            visible: true,
            color: 1,
            forma: Forma::Texto {
                texto: "Texto".into(),
                fuente,
                alto: 10.0,
                espaciado: 0.0,
                afin: Affine::IDENTITY.as_coeffs(),
            },
            puntada: preset("Relleno horizontal"),
        };
        match obj.trazado() {
            Ok(d) => {
                let c = d.bounding_box().center();
                obj.transformar(Affine::translate(Point::ZERO - c));
                if let Some(doc) = &mut self.proyecto {
                    doc.add(vec![obj]);
                }
                self.info("Texto agregado: edítalo en Propiedades");
            }
            Err(e) => {
                self.last_font = None;
                self.error(e);
            }
        }
    }

    /// Cambia la fuente de los textos seleccionados.
    fn change_font(&mut self, path: Option<PathBuf>) {
        let Some(path) = path.or_else(|| self.pick_font()) else {
            return;
        };
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let sel = doc.selection();
        doc.edit(|p| {
            for i in sel {
                if let Forma::Texto { fuente, .. } = &mut p.objetos[i].forma {
                    *fuente = path.clone();
                }
            }
        });
    }
}

/// Puntada de un preset por nombre.
fn preset(name: &str) -> Puntada {
    PRESETS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(Puntada::Ninguna, |(_, p)| p.clone())
}

fn info_section(ui: &mut egui::Ui, doc: &mut crate::proyecto::ProyectoDoc) {
    let jef = &doc.generado.jef;
    let (w, h) = jef.size_mm();
    egui::Grid::new("info").num_columns(2).show(ui, |ui| {
        ui.label("Tamaño");
        ui.label(format!("{w:.1} × {h:.1} mm"));
        ui.end_row();
        ui.label("Puntadas");
        ui.label(jef.count(StitchKind::Normal).to_string());
        ui.end_row();
        ui.label("Saltos");
        ui.label(jef.count(StitchKind::Jump).to_string());
        ui.end_row();
        ui.label("Cambios de hilo");
        ui.label(jef.count(StitchKind::ColorChange).to_string());
        ui.end_row();
    });
    for aviso in &doc.generado.avisos {
        ui.colored_label(WARNING, format!("⚠ {aviso}"));
    }

    let current = Hoop::from_code(doc.proyecto.bastidor);
    let mut chosen = current;
    ui.horizontal(|ui| {
        ui.label("Bastidor");
        egui::ComboBox::from_id_salt("hoop")
            .selected_text(current.name())
            .show_ui(ui, |ui| {
                for h in Hoop::BY_SIZE {
                    let fits = if jef.fits_in(h) { "" } else { "  (no cabe)" };
                    ui.selectable_value(&mut chosen, h, format!("{}{fits}", h.name()));
                }
            });
    });
    if jef.header.hoop != current {
        ui.colored_label(
            WARNING,
            format!("No cabe: se exporta con {}", jef.header.hoop.name()),
        );
    }
    if chosen != current {
        doc.edit(|p| p.bastidor = chosen.code());
    }
}

/// Lista de objetos. Devuelve el objeto cuyo color se quiere cambiar, si alguno.
fn objects_section(ui: &mut egui::Ui, doc: &mut crate::proyecto::ProyectoDoc) -> Option<usize> {
    ui.horizontal(|ui| {
        ui.strong("Objetos");
        if ui.small_button("todos").clicked() {
            doc.seleccion.fill(true);
        }
        if ui.small_button("ninguno").clicked() {
            doc.seleccion.fill(false);
        }
    });
    ui.small("Clic aquí o en el lienzo: seleccionar · Ctrl+clic: agregar · Supr: borrar");
    if doc.proyecto.objetos.is_empty() {
        ui.label("Sin objetos: importa un SVG o agrega un texto.");
    }

    let mut pick = None;
    let mut action: Option<(usize, i8)> = None;
    let mut toggle_visible = None;
    let n = doc.proyecto.objetos.len();
    for i in 0..n {
        let obj = &doc.proyecto.objetos[i];
        let [r, g, b] = jef::palette::thread(obj.color).map_or([128; 3], |t| t.rgb);
        let icon = match obj.forma {
            Forma::Contorno { .. } => "⬟",
            Forma::Linea { .. } => "〰",
            Forma::Texto { .. } => "🔤",
        };
        let mut label = egui::RichText::new(format!("{icon} {}. {}", i + 1, obj.nombre));
        if obj.puntada == Puntada::Ninguna {
            label = label.italics().weak();
        }
        let (mut visible, kind) = (obj.visible, obj.puntada.nombre());
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut visible, "")
                .on_hover_text("Bordar / ocultar")
                .changed()
            {
                toggle_visible = Some(i);
            }
            let swatch = egui::Button::new("")
                .fill(Color32::from_rgb(r, g, b))
                .min_size(Vec2::new(20.0, 18.0));
            if ui.add(swatch).on_hover_text("Cambiar color").clicked() {
                pick = Some(i);
            }
            let response = ui
                .selectable_label(doc.seleccion[i], label)
                .on_hover_text(kind);
            if response.clicked() {
                if ui.input(|inp| inp.modifiers.command) {
                    doc.seleccion[i] = !doc.seleccion[i];
                } else {
                    let only_this = doc.seleccion[i] && doc.selection().len() == 1;
                    doc.seleccion.fill(false);
                    doc.seleccion[i] = !only_this;
                }
            }
            if ui
                .add_enabled(i > 0, egui::Button::new("⏶").small())
                .clicked()
            {
                action = Some((i, -1));
            }
            if ui
                .add_enabled(i + 1 < n, egui::Button::new("⏷").small())
                .clicked()
            {
                action = Some((i, 1));
            }
            if ui
                .add(egui::Button::new("⧉").small())
                .on_hover_text("Duplicar")
                .clicked()
            {
                action = Some((i, 2));
            }
            if ui
                .add(egui::Button::new("🗑").small())
                .on_hover_text("Borrar")
                .clicked()
            {
                action = Some((i, 0));
            }
        });
    }
    if let Some(i) = toggle_visible {
        doc.edit(|p| p.objetos[i].visible = !p.objetos[i].visible);
    }
    match action {
        Some((i, 0)) => doc.remove(i),
        Some((i, 2)) => doc.duplicate(i),
        Some((i, d)) => doc.move_object(i, (i as isize + d as isize) as usize),
        None => {}
    }
    pick
}

/// Nombre, forma y puntada de la selección. Devuelve `Some` si se pidió cambiar la fuente
/// (`Some(None)` = elegir archivo).
fn properties_section(
    ui: &mut egui::Ui,
    doc: &mut crate::proyecto::ProyectoDoc,
) -> Option<Option<PathBuf>> {
    let sel = doc.selection();
    if sel.is_empty() {
        ui.weak("Selecciona objetos para elegir su tipo de puntada.");
        return None;
    }
    ui.strong(if sel.len() == 1 {
        "Propiedades".to_string()
    } else {
        format!("Propiedades ({} objetos)", sel.len())
    });

    // Tipo de puntada: se aplica a toda la selección (el relleno solo a zonas).
    let first = &doc.proyecto.objetos[sel[0]];
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.label("Puntada");
        egui::ComboBox::from_id_salt("preset")
            .selected_text(first.puntada.nombre())
            .show_ui(ui, |ui| {
                for (name, p) in PRESETS {
                    if ui.selectable_label(false, *name).clicked() {
                        chosen = Some(p.clone());
                    }
                }
            });
    });
    if let Some(p) = chosen {
        doc.edit(|proj| {
            for &i in &sel {
                let o = &mut proj.objetos[i];
                if o.es_zona() || !matches!(p, Puntada::Relleno { .. }) {
                    o.puntada = p.clone();
                }
            }
        });
    }
    if sel.iter().any(|&i| !doc.proyecto.objetos[i].es_zona())
        && matches!(
            doc.proyecto.objetos[sel[0]].puntada,
            Puntada::Relleno { .. }
        )
    {
        ui.colored_label(WARNING, "Las líneas no se pueden rellenar");
    }

    if sel.len() > 1 {
        return None;
    }
    let i = sel[0];
    let mut obj = doc.proyecto.objetos[i].clone();
    let mut font = None;
    ui.horizontal(|ui| {
        ui.label("Nombre");
        ui.text_edit_singleline(&mut obj.nombre);
    });
    puntada_params(ui, &mut obj.puntada);
    if let Forma::Texto {
        texto,
        fuente,
        alto,
        espaciado,
        ..
    } = &mut obj.forma
    {
        ui.label("Texto");
        ui.text_edit_multiline(texto);
        ui.horizontal(|ui| {
            ui.label("Alto");
            ui.add(
                egui::DragValue::new(alto)
                    .range(2.0..=200.0)
                    .speed(0.1)
                    .suffix(" mm"),
            );
            ui.label("Espaciado");
            ui.add(
                egui::DragValue::new(espaciado)
                    .range(-5.0..=20.0)
                    .speed(0.05)
                    .suffix(" mm"),
            );
        });
        ui.horizontal(|ui| {
            ui.label(format!("Fuente: {}", file_name(fuente)));
            if ui.button("Cambiar…").clicked() {
                font = Some(None);
            }
        });
        if *alto < 6.0 {
            ui.colored_label(WARNING, "⚠ Texto chico: el relleno puede perder detalle");
        }
    }
    if obj != doc.proyecto.objetos[i] {
        doc.edit_grouped(format!("props {i}"), |p| p.objetos[i] = obj);
    }
    font
}

/// Controles de los parámetros de una puntada.
fn puntada_params(ui: &mut egui::Ui, puntada: &mut Puntada) {
    fn mm(v: &mut f64, range: std::ops::RangeInclusive<f64>) -> egui::DragValue<'_> {
        egui::DragValue::new(v)
            .range(range)
            .speed(0.01)
            .suffix(" mm")
    }
    egui::Grid::new("params")
        .num_columns(2)
        .show(ui, |ui| match puntada {
            Puntada::Ninguna => {}
            Puntada::Corrido { largo, triple } => {
                ui.label("Largo");
                ui.add(mm(largo, 0.5..=6.0));
                ui.end_row();
                ui.label("");
                ui.checkbox(triple, "Triple (más grueso)");
                ui.end_row();
            }
            Puntada::Satin {
                ancho,
                densidad,
                base,
            } => {
                ui.label("Ancho");
                ui.add(mm(ancho, 0.5..=10.0));
                ui.end_row();
                ui.label("Densidad")
                    .on_hover_text("Distancia entre puntadas del mismo lado");
                ui.add(mm(densidad, 0.2..=2.0));
                ui.end_row();
                ui.label("");
                ui.checkbox(base, "Base (pespunte central)");
                ui.end_row();
            }
            Puntada::Relleno {
                angulo,
                separacion,
                largo,
                base,
                compensacion,
            } => {
                ui.label("Ángulo");
                ui.add(
                    egui::DragValue::new(angulo)
                        .range(-180.0..=180.0)
                        .speed(1.0)
                        .suffix("°"),
                );
                ui.end_row();
                ui.label("Separación")
                    .on_hover_text("Distancia entre filas");
                ui.add(mm(separacion, 0.2..=3.0));
                ui.end_row();
                ui.label("Largo");
                ui.add(mm(largo, 1.0..=7.0));
                ui.end_row();
                ui.label("Compensación")
                    .on_hover_text("Alarga las filas para compensar el tirón del hilo");
                ui.add(mm(compensacion, -1.0..=1.0));
                ui.end_row();
                ui.label("");
                ui.checkbox(base, "Base (relleno ralo cruzado)");
                ui.end_row();
            }
        });
}
