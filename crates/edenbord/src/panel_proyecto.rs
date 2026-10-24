//! Panel derecho de un proyecto `.edenbord` (objetos y propiedades), guardar y exportar.

use std::path::PathBuf;

use bordado::booleanas::Booleana;
use bordado::{Fondo, Forma, Objeto, PRESETS, Proyecto, Puntada};
use eframe::egui::{self, Color32, Vec2};
use jef::{Hoop, StitchKind};
use kurbo::{Affine, Point, Shape};

use crate::tema::AVISO as WARNING;
use crate::{Editor, Tool, file_name, iconos, lista, tema, write_verified};

impl Editor {
    /// Panel derecho de un proyecto: objetos arriba, propiedades abajo.
    pub(crate) fn proyecto_panel(&mut self, ui: &mut egui::Ui) {
        let n = self
            .proyecto
            .as_ref()
            .map_or(0, |d| d.proyecto.objetos.len());
        egui::Panel::top("lista_objetos")
            .resizable(true)
            .default_size(260.0)
            .min_size(80.0)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 6)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    tema::titulo(ui, &format!("Objetos · {n}"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let doc = self.proyecto.as_mut().unwrap();
                        if ui.small_button("ninguno").clicked() {
                            doc.seleccion.fill(false);
                        }
                        if ui.small_button("todos").clicked() {
                            doc.seleccion.fill(true);
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("objetos")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let doc = self.proyecto.as_mut().unwrap();
                        if let Some(i) = objects_section(ui, doc) {
                            self.picking_color = Some(i);
                        }
                    })
                    .inner
            });

        egui::ScrollArea::vertical()
            .id_salt("propiedades")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(6.0);
                if self.tool == Tool::Nodes {
                    self.nodes_section(ui);
                    ui.separator();
                }
                self.trace_section(ui);
                let doc = self.proyecto.as_mut().unwrap();
                if let Some(path) = properties_section(ui, doc, &mut self.picking_color) {
                    self.change_font(path);
                }
                ui.separator();
                let doc = self.proyecto.as_ref().unwrap();
                let label = if doc.selection().is_empty() {
                    "Transformar todo"
                } else {
                    "Transformar selección"
                };
                if let Some(op) = self.transform_section(ui, label, false) {
                    crate::apply_proyecto(self.proyecto.as_mut().unwrap(), op);
                }
                ui.separator();
                tema::titulo(ui, "Diseño");
                info_section(ui, self.proyecto.as_mut().unwrap());
                if self.proyecto.as_ref().unwrap().proyecto.fondo.is_some() {
                    ui.separator();
                    self.fondo_section(ui);
                }
            });
    }

    /// Ayuda y acciones de la herramienta de nodos.
    fn nodes_section(&mut self, ui: &mut egui::Ui) {
        tema::titulo(ui, "Nodos");
        let doc = self.proyecto.as_mut().unwrap();
        let Some(i) = doc.editable() else {
            ui.small("Elige un contorno o una línea (clic en el lienzo) para editar sus nodos.");
            return;
        };
        ui.small(
            "Arrastrar: mover nodos o manijas · Shift/Ctrl+clic: agregar · arrastrar en el \
             vacío: elegir varios · doble clic en el trazo: agregar nodo · Supr: borrar",
        );
        let ids = self.nodos_sel.clone();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !ids.is_empty(),
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
                    !ids.is_empty(),
                    egui::Button::new(format!("{} Borrar nodos", iconos::BORRAR)),
                )
                .clicked()
            {
                doc.edit_nodes(i, |subs| crate::nodos::borrar(subs, &ids));
                self.nodos_sel.clear();
            }
        });
    }

    /// Acciones de la pluma y de trazados (combinar, separar, booleanas), si vienen al caso.
    fn trace_section(&mut self, ui: &mut egui::Ui) {
        if self.tool == Tool::Pen {
            tema::titulo(ui, "Pluma");
            ui.small(
                "Clic: vértice en esquina · arrastrar: vértice con curva · clic en el primero: \
                 cerrar zona · Enter: terminar línea · Retroceso o Ctrl+Z: borrar vértice · Esc: \
                 cancelar · botón central o Espacio + arrastrar: mover la vista",
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
        let sel = doc.selection();
        let mut combine = false;
        let mut split = false;
        ui.horizontal(|ui| {
            if sel.len() > 1 {
                combine = ui
                    .button(format!("{} Combinar zonas", iconos::COMBINAR))
                    .on_hover_text(
                        "Une los contornos seleccionados; los interiores quedan como agujeros",
                    )
                    .clicked();
            }
            if sel.iter().any(|&i| doc.subpaths(i) > 1) {
                split = ui
                    .button(format!("{} Separar trazados", iconos::SEPARAR))
                    .on_hover_text(
                        "Cada parte pasa a ser un objeto; los agujeros quedan como zonas aparte \
                         (bórralos o vuelve a combinarlos). Los textos: primero «Convertir en \
                         trazado»",
                    )
                    .clicked();
            }
        });
        if split {
            doc.split();
        }
        if combine && !doc.combine() {
            self.error("Selecciona al menos dos contornos para combinar");
        }

        let doc = self.proyecto.as_mut().unwrap();
        if doc.selection().len() > 1 {
            let mut op = None;
            ui.horizontal(|ui| {
                ui.label("Booleanas");
                for b in Booleana::TODAS {
                    let hover = match b {
                        Booleana::Unir => "Une las zonas en una sola",
                        Booleana::Restar => {
                            "Recorta la primera (la que se borda antes) con las demás, que se \
                             conservan: el fondo no se cose bajo las piezas de encima"
                        }
                        Booleana::Intersecar => "Deja solo lo que tienen en común",
                        Booleana::Excluir => "Deja lo que cubre una sola de las zonas",
                    };
                    if ui.button(b.nombre()).on_hover_text(hover).clicked() {
                        op = Some(b);
                    }
                }
            });
            if let Some(op) = op
                && let Err(e) = doc.boolean(op)
            {
                self.error(e);
            }
        }
    }

    /// Imagen de fondo para calcar.
    fn fondo_section(&mut self, ui: &mut egui::Ui) {
        tema::titulo(ui, "Imagen de fondo");
        let doc = self.proyecto.as_mut().unwrap();
        let mut pick = false;
        ui.horizontal(|ui| {
            pick = ui.button(format!("{} Cambiar…", iconos::IMAGEN)).clicked();
            if ui.button(format!("{} Quitar", iconos::BORRAR)).clicked() {
                doc.edit(|p| p.fondo = None);
            }
        });
        if let Some(fondo) = &doc.proyecto.fondo {
            let mut f = fondo.clone();
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.visible, "Ver");
                ui.checkbox(&mut f.bloqueado, format!("{} Bloquear", iconos::BLOQUEADO))
                    .on_hover_text("Bloqueada, los clics la atraviesan y no se mueve con el ratón");
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

    pub(crate) fn pick_fondo(&mut self) {
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
                bloqueado: false,
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
            opacidad: bordado::OPACIDAD_GUIA,
            bloqueado: false,
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

    pub(crate) fn export_jef(&mut self) {
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

    pub(crate) fn import_svg(&mut self) {
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
    pub(crate) fn add_text(&mut self) {
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
            opacidad: bordado::OPACIDAD_GUIA,
            bloqueado: false,
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
        ui.colored_label(WARNING, format!("{} {aviso}", iconos::AVISO));
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
    if doc.proyecto.objetos.is_empty() {
        ui.weak("Sin objetos: importa un SVG, agrega un texto o dibuja con la pluma.");
        return None;
    }
    let mut pick = None;
    // (objeto, acción): 0 borrar, 1 duplicar, 2 ver/ocultar, 3 bloquear/desbloquear.
    let mut action: Option<(usize, u8)> = None;
    let mut gap = None;
    let n = doc.proyecto.objetos.len();
    for i in 0..n {
        let obj = &doc.proyecto.objetos[i];
        let [r, g, b] = jef::palette::thread(obj.color).map_or([128; 3], |t| t.rgb);
        let icon = match obj.puntada {
            Puntada::Ninguna => iconos::GUIA,
            Puntada::Corrido { .. } => iconos::CORRIDO,
            Puntada::Satin { .. } => iconos::SATIN,
            Puntada::Relleno { .. } => iconos::RELLENO,
        };
        let mut label = egui::RichText::new(&obj.nombre);
        if obj.puntada == Puntada::Ninguna {
            label = label.italics().color(tema::TEXTO_SUAVE);
        }
        if !obj.visible {
            label = label.weak();
        }
        let forma = match obj.forma {
            Forma::Contorno { .. } => "zona",
            Forma::Linea { .. } => "línea",
            Forma::Texto { .. } => "texto",
        };
        let hover = format!(
            "{}. {forma} · {} · arrastrar para cambiar el orden de bordado · clic derecho: más",
            i + 1,
            obj.puntada.nombre()
        );
        let (visible, bloqueado) = (obj.visible, obj.bloqueado);
        let resp = lista::fila(
            ui,
            i,
            doc.seleccion[i],
            |ui| {
                if tema::muestra(ui, Color32::from_rgb(r, g, b))
                    .on_hover_text("Cambiar color")
                    .clicked()
                {
                    pick = Some(i);
                }
                ui.label(tema::icono(icon, false).color(tema::TEXTO_SUAVE));
                ui.add(egui::Label::new(label).truncate().selectable(false));
            },
            |ui| {
                let (ico, ayuda) = if bloqueado {
                    (iconos::BLOQUEADO, "Bloqueado: no se elige en el lienzo")
                } else {
                    (iconos::DESBLOQUEADO, "Bloquear")
                };
                if tema::mini(ui, bloqueado, ico, ayuda).clicked() {
                    action = Some((i, 3));
                }
                let (ico, ayuda) = if visible {
                    (iconos::VISIBLE, "Ocultar (no se borda)")
                } else {
                    (iconos::OCULTO, "Mostrar")
                };
                if tema::mini(ui, !visible, ico, ayuda).clicked() {
                    action = Some((i, 2));
                }
            },
        )
        .on_hover_text(hover);
        if resp.clicked() {
            lista::clic(ui, &mut doc.seleccion, i);
        }
        resp.context_menu(|ui| {
            if ui
                .button(format!("{} Duplicar", iconos::DUPLICAR))
                .clicked()
            {
                action = Some((i, 1));
            }
            if ui.button(format!("{} Borrar", iconos::BORRAR)).clicked() {
                action = Some((i, 0));
            }
        });
        gap = lista::hueco(ui, &resp, i).or(gap);
    }
    if let Some((from, to)) = lista::soltar(ui, gap) {
        doc.move_object(from, to);
    }
    match action {
        Some((i, 0)) => doc.remove(i),
        Some((i, 1)) => doc.duplicate(i),
        Some((i, 2)) => doc.edit(|p| p.objetos[i].visible = !p.objetos[i].visible),
        Some((i, _)) => doc.edit(|p| p.objetos[i].bloqueado = !p.objetos[i].bloqueado),
        None => {}
    }
    pick
}

/// Nombre, forma y puntada de la selección. Devuelve `Some` si se pidió cambiar la fuente
/// (`Some(None)` = elegir archivo).
/// Propiedades de la selección. Con varios objetos, los cambios de parámetros se aplican a
/// los que tienen el mismo tipo de puntada que el primero (solo el valor cambiado).
/// `pick_color` recibe el objeto cuyo color se quiere cambiar.
fn properties_section(
    ui: &mut egui::Ui,
    doc: &mut crate::proyecto::ProyectoDoc,
    pick_color: &mut Option<usize>,
) -> Option<Option<PathBuf>> {
    let sel = doc.selection();
    if sel.is_empty() {
        ui.weak("Selecciona objetos para elegir su tipo de puntada.");
        return None;
    }
    tema::titulo(
        ui,
        &if sel.len() == 1 {
            "Propiedades".to_string()
        } else {
            format!("Propiedades · {} objetos", sel.len())
        },
    );

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
        let [r, g, b] =
            jef::palette::thread(doc.proyecto.objetos[sel[0]].color).map_or([128; 3], |t| t.rgb);
        ui.horizontal(|ui| {
            ui.label("Color");
            let swatch = egui::Button::new("")
                .fill(Color32::from_rgb(r, g, b))
                .min_size(Vec2::new(20.0, 18.0));
            if ui
                .add(swatch)
                .on_hover_text("Cambiar el color de todos")
                .clicked()
            {
                *pick_color = Some(sel[0]);
            }
        });
        let antes = doc.proyecto.objetos[sel[0]].puntada.clone();
        let mismos = sel
            .iter()
            .filter(|&&i| {
                std::mem::discriminant(&doc.proyecto.objetos[i].puntada)
                    == std::mem::discriminant(&antes)
            })
            .count();
        if mismos < sel.len() && antes != Puntada::Ninguna {
            ui.weak(format!(
                "Los cambios se aplican a los {mismos} objetos con «{}»",
                antes.nombre()
            ));
        }
        let guias: Vec<usize> = sel
            .iter()
            .copied()
            .filter(|&i| es_guia_zona(&doc.proyecto.objetos[i]))
            .collect();
        if let Some(&g) = guias.first() {
            let mut opacidad = doc.proyecto.objetos[g].opacidad;
            if opacity_slider(ui, &mut opacidad) {
                doc.edit_grouped("opacidad varios".into(), |p| {
                    for &i in &guias {
                        p.objetos[i].opacidad = opacidad;
                    }
                });
            }
        }
        let mut despues = antes.clone();
        puntada_params(ui, &mut despues);
        if despues != antes {
            doc.edit_grouped("props varios".into(), |p| {
                for &i in &sel {
                    p.objetos[i].puntada.copiar_cambios(&antes, &despues);
                }
            });
        }
        return None;
    }
    let i = sel[0];
    let mut obj = doc.proyecto.objetos[i].clone();
    let mut font = None;
    let mut to_path = false;
    ui.horizontal(|ui| {
        ui.label("Nombre");
        ui.text_edit_singleline(&mut obj.nombre);
    });
    if es_guia_zona(&obj) {
        opacity_slider(ui, &mut obj.opacidad);
    }
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
        if ui
            .add_enabled(
                !doc.lineas[i].is_empty(),
                egui::Button::new("Convertir en trazado"),
            )
            .on_hover_text("Pasa a ser un contorno: se pueden editar sus nodos, ya no el texto")
            .clicked()
        {
            to_path = true;
        }
        if *alto < 6.0 {
            ui.colored_label(
                WARNING,
                format!(
                    "{} Texto chico: el relleno puede perder detalle",
                    iconos::AVISO
                ),
            );
        }
    }
    if obj != doc.proyecto.objetos[i] {
        doc.edit_grouped(format!("props {i}"), |p| p.objetos[i] = obj);
    }
    // El botón solo está activo si el texto ya se pudo trazar: convertir no falla.
    if to_path {
        let _ = doc.text_to_path(i);
    }
    font
}

/// Una guía que es zona: se ve rellena con su color de hilo y esta opacidad.
fn es_guia_zona(o: &Objeto) -> bool {
    o.puntada == Puntada::Ninguna && o.es_zona()
}

/// Opacidad del relleno de una guía. Devuelve si cambió.
fn opacity_slider(ui: &mut egui::Ui, opacidad: &mut f32) -> bool {
    ui.horizontal(|ui| {
        ui.label("Opacidad");
        ui.add(egui::Slider::new(opacidad, 0.0..=1.0))
            .on_hover_text("Cómo se ve el relleno de la guía en el editor; no se borda")
            .changed()
    })
    .inner
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
