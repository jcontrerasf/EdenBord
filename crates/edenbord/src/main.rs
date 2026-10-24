//! EdenBord: editor de archivos JEF y proyectos de bordado `.edenbord`.
//!
//! `cargo run -p edenbord -- [archivo.jef | proyecto.edenbord | dibujo.svg]`

// Sin ventana de consola en Windows (en depuración se deja, para ver los mensajes).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod barras;
mod canvas;
mod contexto;
mod doc;
mod gestos;
mod iconos;
mod lista;
mod nodos;
mod panel_proyecto;
mod pluma;
mod proyecto;
mod tema;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Key, Modifiers, Vec2};
use jef::{Hoop, StitchKind, palette};

use doc::Doc;
use proyecto::ProyectoDoc;

/// Puntadas por segundo de la simulación a velocidad 1×.
const PLAY_RATE: f32 = 400.0;
/// Puntadas más largas que esto (0,1 mm) se marcan como sospechosas.
const LONG_STITCH: i32 = 120;
/// Nombre visible de la aplicación.
const APP: &str = "EdenBord";
/// Identificador para el escritorio (debe coincidir con el `.desktop` y el icono instalado).
const APP_ID: &str = "edenbord";
const ICON: &[u8] = include_bytes!("../../../assets/edenbord-256.png");

fn main() -> eframe::Result {
    let initial = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP)
            .with_app_id(APP_ID)
            .with_icon(eframe::icon_data::from_png_bytes(ICON).expect("icono PNG válido"))
            .with_inner_size([1200.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        APP,
        options,
        Box::new(move |cc| {
            tema::instalar(&cc.egui_ctx);
            let mut app = Editor {
                logo: load_logo(&cc.egui_ctx),
                ..Default::default()
            };
            if let Some(path) = initial {
                app.open(Some(&path));
            }
            Ok(Box::new(app))
        }),
    )
}

/// Acción que espera confirmación del usuario.
enum Pending {
    /// Hay cambios sin guardar y se quiere abrir otro archivo (`None` = proyecto nuevo).
    Discard(Option<PathBuf>),
    /// "Guardar como" apunta al mismo archivo abierto.
    Overwrite(PathBuf),
}

/// Una operación de la sección "Transformar", común a JEF y proyectos.
#[derive(Clone, Copy)]
enum Op {
    /// En 0,1 mm.
    Move(i32, i32),
    Rotate(f64),
    MirrorX,
    MirrorY,
    Scale(f64),
    Center,
}

/// Qué hace el clic en el lienzo de un proyecto.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    /// Editar los nodos de un trazado.
    Nodes,
    /// Calcar: clic agrega vértices, arrastrar los curva.
    Pen,
    /// Medir: arrastrar de un punto a otro.
    Ruler,
}

struct Editor {
    /// JEF abierto. A lo sumo uno de `doc` y `proyecto` está abierto.
    doc: Option<Doc>,
    proyecto: Option<ProyectoDoc>,
    message: Option<(String, bool)>,
    pending: Option<Pending>,
    /// Ventana "Acerca de" abierta.
    about: bool,
    /// Bloque (JEF) u objeto (proyecto) cuyo color se está eligiendo.
    picking_color: Option<usize>,
    /// Última fuente usada para agregar texto.
    last_font: Option<PathBuf>,
    tool: Tool,
    pen: pluma::Pluma,
    /// Medida de la regla: inicio y fin en mm.
    ruler: Option<(kurbo::Point, kurbo::Point)>,
    /// Posición del puntero sobre el lienzo, en mm (para la barra de herramientas).
    pointer_mm: Option<kurbo::Point>,
    /// Imagen de fondo cargada: ruta y textura (`None` si no se pudo leer, para no reintentar).
    fondo_tex: Option<(PathBuf, Option<egui::TextureHandle>)>,
    title: String,
    /// Icono de la aplicación, para la barra y la bienvenida.
    logo: Option<egui::TextureHandle>,

    // Vista.
    /// Escala en píxeles por 0,1 mm y desplazamiento del origen respecto al centro del lienzo.
    scale: f32,
    pan: Vec2,
    fit_pending: bool,
    /// Tamaño del lienzo en el último ajuste, mientras no se haya movido la vista desde entonces.
    fit_size: Option<Vec2>,
    show_jumps: bool,
    show_hoop: bool,
    show_grid: bool,
    show_outlines: bool,
    fabric: Color32,
    /// Gesto del ratón en curso sobre un proyecto (mover, manijas, rectángulo).
    gesto: Option<gestos::Gesto>,
    /// La imagen de fondo está elegida (muestra sus manijas) en vez de objetos.
    fondo_sel: bool,
    /// Herramienta de nodos: objeto en edición, nodos elegidos y arrastre en curso.
    nodos_obj: Option<usize>,
    nodos_sel: Vec<nodos::Id>,
    gesto_nodo: Option<canvas::GestoNodo>,
    /// El próximo pegado va en el mismo lugar (Ctrl+Shift+V), no en el puntero.
    paste_in_place: bool,
    /// JEF: arrastre de la selección con Shift; resto en 0,1 mm aún no aplicado.
    moving: bool,
    drag_rest: Vec2,

    // Herramientas.
    step_mm: f32,
    angle: f64,
    scale_pct: f64,
    /// Ancho y alto proporcionales en la barra contextual.
    lock_ratio: bool,

    // Simulación: cuántas entradas de la secuencia se dibujan.
    progress: usize,
    playing: bool,
    speed: f32,
    play_accum: f32,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            doc: None,
            proyecto: None,
            message: None,
            pending: None,
            about: false,
            picking_color: None,
            last_font: None,
            tool: Tool::Select,
            pen: pluma::Pluma::default(),
            ruler: None,
            pointer_mm: None,
            fondo_tex: None,
            title: String::new(),
            logo: None,
            scale: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            fit_size: None,
            show_jumps: false,
            show_hoop: true,
            show_grid: true,
            show_outlines: false,
            fabric: Color32::from_rgb(196, 190, 178),
            gesto: None,
            fondo_sel: false,
            nodos_obj: None,
            nodos_sel: vec![],
            gesto_nodo: None,
            paste_in_place: false,
            moving: false,
            drag_rest: Vec2::ZERO,
            step_mm: 1.0,
            angle: 15.0,
            scale_pct: 100.0,
            lock_ratio: true,
            progress: usize::MAX,
            playing: false,
            speed: 1.0,
            play_accum: 0.0,
        }
    }
}

fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let rgba = image::load_from_memory(ICON).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let img = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture("logo", img, egui::TextureOptions::LINEAR))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn jef_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Bordado Janome", &["jef", "JEF"])
}

fn open_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new()
        .add_filter(
            "Bordados, proyectos y vectores",
            &["jef", "JEF", bordado::EXTENSION, "bordado", "svg", "SVG"],
        )
        .add_filter("Bordado Janome", &["jef", "JEF"])
        .add_filter("Proyecto EdenBord", &[bordado::EXTENSION, "bordado"])
        .add_filter("Vector SVG", &["svg", "SVG"])
}

fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}

/// Escribe el JEF y lo relee para confirmar que quedó bien.
fn write_verified(path: &Path, jef: &jef::Jef) -> Result<(), String> {
    let bytes = jef::write(jef);
    std::fs::write(path, &bytes).map_err(|e| e.to_string())?;
    let back = std::fs::read(path).map_err(|e| e.to_string())?;
    let again = jef::read(&back).map_err(|e| e.to_string())?;
    if back == bytes && jef::write(&again) == bytes {
        Ok(())
    } else {
        Err("el archivo releído no coincide".into())
    }
}

impl Editor {
    fn info(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), false));
    }

    fn error(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), true));
    }

    fn dirty(&self) -> bool {
        self.doc.as_ref().is_some_and(|d| d.dirty)
            || self.proyecto.as_ref().is_some_and(|d| d.dirty)
    }

    /// Abre un archivo (o un proyecto nuevo si `path` es `None`), pidiendo confirmación si
    /// hay cambios sin guardar.
    fn request_open(&mut self, path: Option<PathBuf>) {
        if self.dirty() {
            self.pending = Some(Pending::Discard(path));
        } else {
            self.open(path.as_deref());
        }
    }

    /// Abre según la extensión: `.edenbord` (o el antiguo `.bordado`) proyecto, `.svg` proyecto nuevo con sus trazados
    /// como guías, cualquier otra cosa como JEF.
    fn open(&mut self, path: Option<&Path>) {
        let Some(path) = path else {
            self.set_proyecto(ProyectoDoc::new(
                None,
                "proyecto nuevo".into(),
                bordado::Proyecto::default(),
            ));
            return;
        };
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let result = match extension(path).as_str() {
            "edenbord" | "bordado" => bordado::Proyecto::leer_archivo(path)
                .map(|p| self.set_proyecto(ProyectoDoc::new(Some(path.into()), stem, p)))
                .map_err(|e| e.to_string()),
            "svg" => bordado::svg::importar_archivo(path)
                .map(|objetos| {
                    let n = objetos.len();
                    let proyecto = bordado::Proyecto {
                        objetos,
                        ..Default::default()
                    };
                    self.set_proyecto(ProyectoDoc::new(None, stem, proyecto));
                    self.info(format!(
                        "{n} trazados importados como guías: elige uno y dale un tipo de puntada"
                    ));
                })
                .map_err(|e| e.to_string()),
            _ => jef::read_file(path)
                .map(|jef| {
                    self.reset_view();
                    self.doc = Some(Doc::new(path.to_path_buf(), jef));
                    self.proyecto = None;
                })
                .map_err(|e| e.to_string()),
        };
        if let Err(e) = result {
            self.error(format!("No se pudo abrir {}: {e}", path.display()));
        }
    }

    fn set_proyecto(&mut self, doc: ProyectoDoc) {
        self.reset_view();
        self.proyecto = Some(doc);
        self.doc = None;
    }

    fn reset_view(&mut self) {
        self.message = None;
        self.pen = pluma::Pluma::default();
        self.ruler = None;
        self.gesto = None;
        self.fondo_sel = false;
        self.nodos_obj = None;
        self.nodos_sel.clear();
        self.gesto_nodo = None;
        self.tool = Tool::Select;
        self.picking_color = None;
        self.playing = false;
        self.progress = usize::MAX;
        self.fit_pending = true;
    }

    /// El diseño que se dibuja y simula: el JEF abierto o las puntadas del proyecto.
    fn current_jef(&self) -> Option<&jef::Jef> {
        self.doc
            .as_ref()
            .map(|d| &d.jef)
            .or(self.proyecto.as_ref().map(|p| &p.generado.jef))
    }

    fn add_design(&mut self) {
        let Some(path) = jef_dialog().pick_file() else {
            return;
        };
        match jef::read_file(&path) {
            Ok(other) => {
                if let Some(doc) = &mut self.doc {
                    doc.append(&other);
                    self.info(format!(
                        "{} agregado; quedó seleccionado para moverlo",
                        file_name(&path)
                    ));
                }
            }
            Err(e) => self.error(format!("No se pudo abrir {}: {e}", path.display())),
        }
    }

    fn save_as(&mut self) {
        let Some(doc) = &self.doc else {
            return;
        };
        let stem = doc.path.file_stem().unwrap_or_default().to_string_lossy();
        let mut dialog = jef_dialog().set_file_name(format!("{stem}_editado.jef"));
        if let Some(dir) = doc.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        if path == doc.path {
            self.pending = Some(Pending::Overwrite(path));
        } else {
            self.write_to(path);
        }
    }

    /// Escribe el diseño y lo relee para confirmar que quedó bien.
    fn write_to(&mut self, path: PathBuf) {
        let Some(doc) = &mut self.doc else {
            return;
        };
        match write_verified(&path, &doc.jef) {
            Ok(()) => {
                doc.path = path.clone();
                doc.dirty = false;
                self.info(format!("Guardado y verificado: {}", file_name(&path)));
            }
            Err(e) => self.error(format!("Error al guardar {}: {e}", path.display())),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let (undo, redo, save, arrows) = ctx.input_mut(|i| {
            let undo = i.consume_key(Modifiers::COMMAND, Key::Z);
            let redo =
                i.consume_key(Modifiers::COMMAND, Key::Y) || i.consume_key(ctrl_shift, Key::Z);
            let save = i.consume_key(Modifiers::COMMAND, Key::S);
            let arrows = [
                (Key::ArrowLeft, -1, 0),
                (Key::ArrowRight, 1, 0),
                (Key::ArrowUp, 0, -1),
                (Key::ArrowDown, 0, 1),
            ]
            .into_iter()
            .filter(|&(k, _, _)| i.consume_key(Modifiers::NONE, k))
            .fold((0, 0), |acc, (_, dx, dy)| (acc.0 + dx, acc.1 + dy));
            (undo, redo, save, arrows)
        });
        // Herramientas: S seleccionar, N nodos, P pluma, R regla (pluma y nodos solo en proyectos).
        let tool = ctx.input_mut(|i| {
            [
                (Key::S, Tool::Select),
                (Key::N, Tool::Nodes),
                (Key::P, Tool::Pen),
                (Key::R, Tool::Ruler),
            ]
            .into_iter()
            .find(|&(k, _)| i.consume_key(Modifiers::NONE, k))
            .map(|(_, t)| t)
        });
        if let Some(t) = tool
            && (self.proyecto.is_some() || matches!(t, Tool::Select | Tool::Ruler))
            && self.gesto.is_none()
            && self.gesto_nodo.is_none()
        {
            self.set_tool(t);
        }
        // Mientras se dibuja con la pluma, deshacer quita el último vértice (no toca el proyecto).
        let pen_drawing = self.tool == Tool::Pen && !self.pen.nodos.is_empty();
        let (undo, redo) = if pen_drawing && undo {
            self.pen.nodos.pop();
            (false, false)
        } else {
            (undo, redo)
        };
        // Durante un gesto, Esc lo cancela y deshacer espera a que termine.
        let (undo, redo) = if self.gesto_nodo.is_some() {
            (false, false)
        } else if self.gesto.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.gesto = None;
            }
            (false, false)
        } else {
            (undo, redo)
        };
        if pen_drawing {
            let (enter, escape, back) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::Enter),
                    i.consume_key(Modifiers::NONE, Key::Escape),
                    i.consume_key(Modifiers::NONE, Key::Backspace),
                )
            });
            if enter {
                self.finish_pen(false);
            }
            if escape {
                self.pen = pluma::Pluma::default();
            }
            if back {
                self.pen.nodos.pop();
            }
        }
        if self.ruler.is_some() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.ruler = None;
        }
        let delete = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Delete));
        if save {
            if self.proyecto.is_some() {
                self.save_project(false);
            } else {
                self.save_as();
            }
        }
        let step = (self.step_mm * 10.0).round() as i32;
        let op = (arrows != (0, 0)).then_some(Op::Move(arrows.0 * step, arrows.1 * step));
        if let Some(doc) = &mut self.doc {
            if undo {
                doc.undo();
            }
            if redo {
                doc.redo();
            }
            if let Some(op) = op {
                apply_jef(doc, op);
            }
        }
        if let Some(doc) = &mut self.proyecto {
            if undo {
                doc.undo();
            }
            if redo {
                doc.redo();
            }
            if let Some(op) = op {
                apply_proyecto(doc, op);
            }
            if delete && self.tool == Tool::Nodes && !self.nodos_sel.is_empty() {
                if let Some(i) = self.nodos_obj {
                    let ids = std::mem::take(&mut self.nodos_sel);
                    doc.edit_nodes(i, |subs| nodos::borrar(subs, &ids));
                }
            } else if delete && self.fondo_sel {
                doc.edit(|p| p.fondo = None);
                self.fondo_sel = false;
            } else if delete {
                doc.remove_selected();
            }
        }
        self.clipboard_shortcuts(ctx);
    }

    /// Ctrl+C/X/V (egui los entrega como eventos), Ctrl+D, Ctrl+A y Esc en proyectos.
    fn clipboard_shortcuts(&mut self, ctx: &egui::Context) {
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let (copy, cut, paste, in_place, duplicate, all, escape) = ctx.input_mut(|i| {
            let mut copy = false;
            let mut cut = false;
            let mut paste = None;
            for e in &i.events {
                match e {
                    egui::Event::Copy => copy = true,
                    egui::Event::Cut => cut = true,
                    egui::Event::Paste(text) => paste = Some(text.clone()),
                    _ => {}
                }
            }
            (
                copy,
                cut,
                paste,
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::V),
                i.consume_key(Modifiers::COMMAND, Key::D),
                i.consume_key(Modifiers::COMMAND, Key::A),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if (copy || cut) && !doc.selection().is_empty() {
            ctx.copy_text(bordado::copiar_objetos(&doc.selected_objects()));
            if cut {
                doc.remove_selected();
            }
        }
        // Ctrl+Shift+V no es un evento de pegar: se pide el portapapeles y llega después.
        if in_place {
            self.paste_in_place = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        if let Some(text) = paste {
            // Al puntero si está sobre el lienzo; con Ctrl+Shift+V, en el mismo lugar.
            let centro = if self.paste_in_place {
                None
            } else {
                self.pointer_mm
            };
            self.paste_in_place = false;
            if let Some(objetos) = bordado::pegar_objetos(&text) {
                doc.paste(objetos, centro);
                self.fondo_sel = false;
            }
        }
        if duplicate {
            doc.duplicate_selected();
        }
        if all {
            doc.select_all();
            self.fondo_sel = false;
        }
        if escape && !self.nodos_sel.is_empty() {
            self.nodos_sel.clear();
        } else if escape {
            doc.seleccion.fill(false);
            self.fondo_sel = false;
        }
    }

    /// Panel derecho: lista (colores u objetos) arriba y propiedades abajo.
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        if self.proyecto.is_some() {
            self.proyecto_panel(ui);
            return;
        }
        let Some(doc) = &mut self.doc else {
            return;
        };
        let n = doc.blocks.len();
        egui::Panel::top("lista_colores")
            .resizable(true)
            .default_size(260.0)
            .min_size(80.0)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 6)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    tema::titulo(ui, &format!("Colores · {n}"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("ninguno").clicked() {
                            doc.selected.fill(false);
                        }
                        if ui.small_button("todos").clicked() {
                            doc.selected.fill(true);
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("colores")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        if let Some(i) = Self::color_section(ui, doc) {
                            self.picking_color = Some(i);
                        }
                    });
            });

        egui::ScrollArea::vertical()
            .id_salt("propiedades")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(6.0);
                tema::titulo(ui, "Diseño");
                let doc = self.doc.as_mut().unwrap();
                Self::info_section(ui, doc);
                Self::hoop_section(ui, doc);
            });
    }

    fn info_section(ui: &mut egui::Ui, doc: &Doc) {
        let jef = &doc.jef;
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
        });
        let (_, long) = jef.odd_stitches(0, LONG_STITCH);
        if long > 0 {
            ui.colored_label(
                tema::AVISO,
                format!("{} {long} puntadas de más de 12 mm", iconos::AVISO),
            );
        }
        if !jef.trailing.is_empty() {
            ui.colored_label(
                tema::AVISO,
                format!(
                    "{} {} bytes extra tras el fin",
                    iconos::AVISO,
                    jef.trailing.len()
                ),
            );
        }
    }

    fn hoop_section(ui: &mut egui::Ui, doc: &mut Doc) {
        let current = doc.jef.header.hoop;
        let mut chosen = current;
        ui.horizontal(|ui| {
            ui.label("Bastidor");
            egui::ComboBox::from_id_salt("hoop")
                .selected_text(current.name())
                .show_ui(ui, |ui| {
                    for h in Hoop::BY_SIZE {
                        let fits = if doc.jef.fits_in(h) {
                            ""
                        } else {
                            "  (no cabe)"
                        };
                        ui.selectable_value(&mut chosen, h, format!("{}{fits}", h.name()));
                    }
                });
        });
        if chosen != current {
            doc.edit(|j| {
                j.header.hoop = chosen;
                j.refresh_header();
            });
        }
        if !doc.jef.fits_in(doc.jef.header.hoop) {
            ui.colored_label(tema::COPIHUE, "El diseño no cabe en este bastidor");
        }
    }

    /// Lista de bloques de color (arrastrar = cambiar el orden de bordado). Devuelve el
    /// bloque cuyo color se quiere cambiar, si alguno.
    fn color_section(ui: &mut egui::Ui, doc: &mut Doc) -> Option<usize> {
        let mut pick = None;
        // (bloque, acción): 0 borrar, 1 ver/ocultar, -1/2 subir/bajar.
        let mut action: Option<(usize, i8)> = None;
        let mut gap = None;
        let n = doc.blocks.len();
        for i in 0..n {
            let idx = doc.jef.colors.get(i).copied().unwrap_or(0);
            let name = palette::thread(idx).map_or("?", |t| t.name);
            let [r, g, b] = doc.jef.thread_rgb(i);
            let visible = doc.visible[i];
            let resp = lista::fila(
                ui,
                i,
                doc.selected[i],
                |ui| {
                    if tema::muestra(ui, Color32::from_rgb(r, g, b))
                        .on_hover_text("Cambiar color")
                        .clicked()
                    {
                        pick = Some(i);
                    }
                    let mut text = egui::RichText::new(format!("{}. {name}", i + 1));
                    if !visible {
                        text = text.weak();
                    }
                    ui.add(egui::Label::new(text).truncate().selectable(false));
                    ui.weak(format!("n.º {idx}"));
                },
                |ui| {
                    let (ico, ayuda) = if visible {
                        (iconos::VISIBLE, "Ocultar")
                    } else {
                        (iconos::OCULTO, "Mostrar")
                    };
                    if tema::mini(ui, !visible, ico, ayuda).clicked() {
                        action = Some((i, 1));
                    }
                },
            )
            .on_hover_text(format!(
                "{} entradas · arrastrar para cambiar el orden de bordado · clic derecho: más",
                doc.blocks[i].len()
            ));
            if resp.clicked() {
                lista::clic(ui, &mut doc.selected, i);
            }
            resp.context_menu(|ui| {
                if ui
                    .add_enabled(i > 0, egui::Button::new(format!("{} Subir", iconos::SUBIR)))
                    .clicked()
                {
                    action = Some((i, -1));
                }
                if ui
                    .add_enabled(
                        i + 1 < n,
                        egui::Button::new(format!("{} Bajar", iconos::BAJAR)),
                    )
                    .clicked()
                {
                    action = Some((i, 2));
                }
                if ui
                    .add_enabled(
                        n > 1,
                        egui::Button::new(format!("{} Borrar bloque", iconos::BORRAR)),
                    )
                    .clicked()
                {
                    action = Some((i, 0));
                }
            });
            gap = lista::hueco(ui, &resp, i).or(gap);
        }
        if let Some((from, to)) = lista::soltar(ui, gap) {
            doc.move_block(from, to);
        }
        match action {
            Some((i, 0)) => doc.remove_block(i),
            Some((i, 1)) => doc.visible[i] = !doc.visible[i],
            Some((i, -1)) => doc.move_block(i, i - 1),
            Some((i, _)) => doc.move_block(i, i + 1),
            None => {}
        }
        pick
    }

    /// Ventana para elegir un hilo de la paleta Janome.
    fn color_picker(&mut self, ctx: &egui::Context) {
        let Some(target) = self.picking_color else {
            return;
        };
        let (title, current) = if let Some(doc) = &self.doc {
            (
                format!("Color del bloque {}", target + 1),
                doc.jef.colors.get(target).copied(),
            )
        } else if let Some(doc) = &self.proyecto {
            let obj = doc.proyecto.objetos.get(target);
            let n = doc.selection().len();
            let title = if n > 1 && doc.seleccion.get(target) == Some(&true) {
                format!("Color de {n} objetos")
            } else {
                format!("Color de «{}»", obj.map_or("", |o| o.nombre.as_str()))
            };
            (title, obj.map(|o| o.color))
        } else {
            (String::new(), None)
        };
        let Some(current) = current else {
            self.picking_color = None;
            return;
        };
        let mut open = true;
        let mut chosen = None;
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("palette")
                    .spacing([4.0, 4.0])
                    .show(ui, |ui| {
                        for (i, t) in palette::PALETTE.iter().enumerate().skip(1) {
                            let [r, g, b] = t.rgb;
                            let mut button = egui::Button::new("")
                                .fill(Color32::from_rgb(r, g, b))
                                .min_size(Vec2::new(26.0, 22.0));
                            if i as i32 == current {
                                button = button.stroke(egui::Stroke::new(2.5, tema::TEXTO));
                            }
                            let hover = format!("{} — Janome {} (n.º {i})", t.name, t.catalog);
                            if ui.add(button).on_hover_text(hover).clicked() {
                                chosen = Some(i as i32);
                            }
                            if i % 10 == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
        if let Some(c) = chosen {
            if let Some(doc) = &mut self.doc
                && c != current
            {
                doc.edit(|j| j.set_color(target, c));
            }
            if let Some(doc) = &mut self.proyecto {
                // Si el objeto es parte de la selección, el color va a toda la selección.
                let sel = match doc.selection() {
                    sel if sel.contains(&target) => sel,
                    _ => vec![target],
                };
                if sel.iter().any(|&i| doc.proyecto.objetos[i].color != c) {
                    doc.edit(|p| {
                        for i in sel {
                            p.objetos[i].color = c;
                        }
                    });
                }
            }
            open = false;
        }
        if !open {
            self.picking_color = None;
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else {
            return;
        };
        let (title, text, ok) = match pending {
            Pending::Discard(_) => (
                "Cambios sin guardar",
                "Hay cambios sin guardar. ¿Descartarlos y abrir el otro archivo?",
                "Descartar y abrir",
            ),
            Pending::Overwrite(_) => (
                "Sobrescribir",
                "Vas a sobrescribir el archivo abierto. ¿Seguro?",
                "Sobrescribir",
            ),
        };
        let mut answer = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button(ok).clicked() {
                        answer = Some(true);
                    }
                    if ui.button("Cancelar").clicked() {
                        answer = Some(false);
                    }
                });
            });
        if let Some(yes) = answer {
            let pending = self.pending.take();
            if yes {
                match pending {
                    Some(Pending::Discard(path)) => self.open(path.as_deref()),
                    Some(Pending::Overwrite(path)) => self.write_to(path),
                    None => {}
                }
            }
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new(format!("Acerca de {APP}"))
            .open(&mut self.about)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.heading(format!("{APP} {}", env!("CARGO_PKG_VERSION")));
                ui.label("Editor de bordados para máquinas Janome (JEF).");
                ui.add_space(6.0);
                ui.label(
                    "Eden viene del himno de Chile: «y tu campo de flores bordado es la copia \
                     feliz del Edén». Bord, de bordado.",
                );
                ui.add_space(6.0);
                ui.label("Software libre bajo la licencia GPL-3.0 o posterior.");
            });
    }

    /// Nombre del documento abierto, si hay uno.
    fn doc_name(&self) -> Option<String> {
        match (&self.doc, &self.proyecto) {
            (Some(doc), _) => Some(file_name(&doc.path)),
            (_, Some(doc)) => Some(doc.path.as_deref().map_or(doc.nombre.clone(), file_name)),
            _ => None,
        }
    }

    /// Si se puede deshacer y rehacer en el documento abierto.
    fn can_undo_redo(&self) -> (bool, bool) {
        match (&self.doc, &self.proyecto) {
            (Some(d), _) => (d.can_undo(), d.can_redo()),
            (_, Some(d)) => (d.can_undo(), d.can_redo()),
            _ => (false, false),
        }
    }

    fn undo(&mut self) {
        if let Some(d) = &mut self.doc {
            d.undo();
        }
        if let Some(d) = &mut self.proyecto {
            d.undo();
        }
    }

    fn redo(&mut self) {
        if let Some(d) = &mut self.doc {
            d.redo();
        }
        if let Some(d) = &mut self.proyecto {
            d.redo();
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.doc_name() {
            Some(name) => format!("{}{name} — EdenBord", if self.dirty() { "• " } else { "" },),
            None => "EdenBord".into(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

impl eframe::App for Editor {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            self.request_open(Some(path));
        }
        if self.pending.is_none() {
            self.shortcuts(ctx);
        }

        if self.playing
            && let Some(jef) = self.current_jef()
        {
            let total = jef.stitches.len();
            self.play_accum += ctx.input(|i| i.stable_dt) * PLAY_RATE * self.speed;
            let step = self.play_accum as usize;
            self.play_accum -= step as f32;
            self.progress = self.progress.min(total) + step;
            if self.progress >= total {
                self.progress = usize::MAX;
                self.playing = false;
            }
            ctx.request_repaint();
        }
        // Una edición agrupada (arrastrar un valor, escribir) termina al soltar el ratón y
        // dejar el campo: la siguiente, aunque sea del mismo campo, es otro deshacer.
        let idle = ctx.input(|i| !i.pointer.any_down()) && ctx.memory(|m| m.focused()).is_none();
        if idle && let Some(doc) = &mut self.proyecto {
            doc.end_group();
        }
        if idle && let Some(doc) = &mut self.doc {
            doc.end_group();
        }
        self.update_title(ctx);
        self.load_fondo(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let barra = egui::Frame::NONE
            .fill(tema::LINO)
            .inner_margin(egui::Margin::symmetric(10, 6));
        egui::Panel::top("barra")
            .frame(barra)
            .show(ui, |ui| self.top_bar(ui));
        if self.doc.is_some() || self.proyecto.is_some() {
            egui::Panel::top("contexto")
                .frame(
                    egui::Frame::NONE
                        .fill(tema::LINO_CLARO)
                        .inner_margin(egui::Margin::symmetric(10, 4)),
                )
                .show(ui, |ui| self.context_bar(ui));
            egui::Panel::bottom("estado")
                .frame(barra)
                .show(ui, |ui| self.status_bar(ui));
            egui::Panel::left("herramientas")
                .resizable(false)
                .exact_size(46.0)
                .frame(egui::Frame::NONE.fill(tema::LINO).inner_margin(6))
                .show(ui, |ui| self.tools_bar(ui));
            egui::Panel::right("panel")
                .resizable(true)
                .default_size(300.0)
                .min_size(240.0)
                .frame(
                    egui::Frame::NONE
                        .fill(tema::LINO)
                        .inner_margin(egui::Margin::symmetric(10, 0)),
                )
                .show(ui, |ui| self.side_panel(ui));
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(tema::LINO))
                .show(ui, |ui| self.canvas(ui));
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(tema::LINO))
                .show(ui, |ui| self.welcome(ui));
        }
        self.color_picker(&ctx);
        self.confirm_dialog(&ctx);
        self.about_dialog(&ctx);
    }
}

fn apply_jef(doc: &mut Doc, op: Op) {
    let sel = doc.selection();
    doc.edit(|j| match op {
        Op::Move(dx, dy) => j.translate(&sel, dx, dy),
        Op::Rotate(deg) => j.rotate(&sel, deg),
        Op::MirrorX => j.mirror_x(&sel),
        Op::MirrorY => j.mirror_y(&sel),
        Op::Scale(f) => j.scale(&sel, f),
        Op::Center => j.center_on_hoop(&sel),
    });
}

fn apply_proyecto(doc: &mut ProyectoDoc, op: Op) {
    use kurbo::{Affine, Point, Vec2};
    let sel = doc.selection();
    doc.transform(&sel, |r| {
        let c = r.center();
        let about =
            |t: Affine| Affine::translate(c.to_vec2()) * t * Affine::translate(-c.to_vec2());
        match op {
            Op::Move(dx, dy) => Affine::translate(Vec2::new(f64::from(dx), f64::from(dy)) / 10.0),
            Op::Rotate(deg) => Affine::rotate_about(deg.to_radians(), c),
            Op::MirrorX => about(Affine::FLIP_X),
            Op::MirrorY => about(Affine::FLIP_Y),
            Op::Scale(f) => Affine::scale_about(f, c),
            Op::Center => Affine::translate(Point::ZERO - c),
        }
    });
}
