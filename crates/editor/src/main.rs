//! Editor de archivos JEF.
//!
//! `cargo run -p editor -- [archivo.jef]`

mod canvas;
mod doc;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Key, Modifiers, Vec2};
use jef::{Hoop, StitchKind, palette};

use doc::Doc;

/// Puntadas por segundo de la simulación a velocidad 1×.
const PLAY_RATE: f32 = 400.0;
/// Puntadas más largas que esto (0,1 mm) se marcan como sospechosas.
const LONG_STITCH: i32 = 120;

fn main() -> eframe::Result {
    let initial = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Editor JEF")
            .with_inner_size([1200.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Editor JEF",
        options,
        Box::new(move |_cc| {
            let mut app = Editor::default();
            if let Some(path) = initial {
                app.open(&path);
            }
            Ok(Box::new(app))
        }),
    )
}

/// Acción que espera confirmación del usuario.
enum Pending {
    /// Hay cambios sin guardar y se quiere abrir otro archivo.
    Discard(PathBuf),
    /// "Guardar como" apunta al mismo archivo abierto.
    Overwrite(PathBuf),
}

struct Editor {
    doc: Option<Doc>,
    message: Option<(String, bool)>,
    pending: Option<Pending>,
    /// Bloque cuyo color se está eligiendo.
    picking_color: Option<usize>,
    title: String,

    // Vista.
    /// Escala en píxeles por 0,1 mm y desplazamiento del origen respecto al centro del lienzo.
    scale: f32,
    pan: Vec2,
    fit_pending: bool,
    show_jumps: bool,
    show_hoop: bool,
    fabric: Color32,
    /// Arrastre de la selección con Shift: resto en 0,1 mm aún no aplicado.
    moving: bool,
    drag_rest: Vec2,

    // Herramientas.
    step_mm: f32,
    angle: f64,
    scale_pct: f64,

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
            message: None,
            pending: None,
            picking_color: None,
            title: String::new(),
            scale: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            show_jumps: false,
            show_hoop: true,
            fabric: Color32::from_rgb(196, 190, 178),
            moving: false,
            drag_rest: Vec2::ZERO,
            step_mm: 1.0,
            angle: 15.0,
            scale_pct: 100.0,
            progress: usize::MAX,
            playing: false,
            speed: 1.0,
            play_accum: 0.0,
        }
    }
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

impl Editor {
    fn info(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), false));
    }

    fn error(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), true));
    }

    /// Abre un archivo, pidiendo confirmación si hay cambios sin guardar.
    fn request_open(&mut self, path: PathBuf) {
        if self.doc.as_ref().is_some_and(|d| d.dirty) {
            self.pending = Some(Pending::Discard(path));
        } else {
            self.open(&path);
        }
    }

    fn open(&mut self, path: &Path) {
        match jef::read_file(path) {
            Ok(jef) => {
                self.doc = Some(Doc::new(path.to_path_buf(), jef));
                self.message = None;
                self.picking_color = None;
                self.playing = false;
                self.progress = usize::MAX;
                self.fit_pending = true;
            }
            Err(e) => self.error(format!("No se pudo abrir {}: {e}", path.display())),
        }
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
        let bytes = jef::write(&doc.jef);
        let result = std::fs::write(&path, &bytes)
            .map_err(|e| e.to_string())
            .and_then(|()| std::fs::read(&path).map_err(|e| e.to_string()))
            .and_then(|back| {
                let again = jef::read(&back).map_err(|e| e.to_string())?;
                if back == bytes && jef::write(&again) == bytes {
                    Ok(())
                } else {
                    Err("el archivo releído no coincide".into())
                }
            });
        match result {
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
        if save {
            self.save_as();
        }
        let step = (self.step_mm * 10.0).round() as i32;
        let Some(doc) = &mut self.doc else {
            return;
        };
        if undo {
            doc.undo();
        }
        if redo {
            doc.redo();
        }
        if arrows != (0, 0) {
            let sel = doc.selection();
            doc.edit(|j| j.translate(&sel, arrows.0 * step, arrows.1 * step));
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("📂 Abrir…").clicked()
                && let Some(path) = jef_dialog().pick_file()
            {
                self.request_open(path);
            }
            let has_doc = self.doc.is_some();
            if ui
                .add_enabled(has_doc, egui::Button::new("💾 Guardar como…"))
                .on_hover_text("Ctrl+S")
                .clicked()
            {
                self.save_as();
            }
            if ui
                .add_enabled(has_doc, egui::Button::new("➕ Agregar diseño…"))
                .clicked()
            {
                self.add_design();
            }
        });
        if let Some((text, is_error)) = &self.message {
            let color = if *is_error {
                Color32::RED
            } else {
                ui.visuals().text_color()
            };
            ui.colored_label(color, text);
        }
        ui.separator();

        let Some(doc) = &mut self.doc else {
            ui.label("Abre un archivo .jef o arrástralo a la ventana.");
            return;
        };

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

        egui::ScrollArea::vertical().show(ui, |ui| {
            Self::info_section(ui, doc);
            ui.separator();
            Self::hoop_section(ui, doc);
            ui.separator();
            self.picking_color = Self::color_section(ui, doc).or(self.picking_color);
            ui.separator();
            Self::transform_section(
                ui,
                doc,
                &mut self.step_mm,
                &mut self.angle,
                &mut self.scale_pct,
            );
            ui.separator();
            ui.collapsing("Vista", |ui| {
                ui.checkbox(&mut self.show_jumps, "Mostrar saltos");
                ui.checkbox(&mut self.show_hoop, "Mostrar bastidor");
                ui.horizontal(|ui| {
                    ui.label("Tela");
                    ui.color_edit_button_srgba(&mut self.fabric);
                });
                if ui.button("Ajustar a la ventana").clicked() {
                    self.fit_pending = true;
                }
            });
            ui.collapsing("Simulación", |ui| {
                // `usize::MAX` = mostrar todo, aunque el diseño crezca al editarlo.
                let total = doc.jef.stitches.len();
                let mut shown = self.progress.min(total);
                ui.horizontal(|ui| {
                    let label = if self.playing {
                        "⏸ Pausa"
                    } else {
                        "▶ Reproducir"
                    };
                    if ui.button(label).clicked() {
                        if !self.playing && shown >= total {
                            shown = 0;
                        }
                        self.playing = !self.playing;
                    }
                    if ui.button("⏮").clicked() {
                        shown = 0;
                    }
                    if ui.button("⏭").clicked() {
                        shown = total;
                        self.playing = false;
                    }
                });
                ui.add(egui::Slider::new(&mut shown, 0..=total).text("puntada"));
                self.progress = if shown >= total { usize::MAX } else { shown };
                ui.add(
                    egui::Slider::new(&mut self.speed, 0.25..=20.0)
                        .logarithmic(true)
                        .text("velocidad"),
                );
            });
        });
    }

    fn info_section(ui: &mut egui::Ui, doc: &Doc) {
        let jef = &doc.jef;
        let mut name = file_name(&doc.path);
        if doc.dirty {
            name.push_str(" •");
        }
        ui.heading(name);
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
            ui.label("Colores");
            ui.label(jef.colors.len().to_string());
            ui.end_row();
        });
        let (_, long) = jef.odd_stitches(0, LONG_STITCH);
        if long > 0 {
            ui.colored_label(
                Color32::from_rgb(200, 120, 0),
                format!("⚠ {long} puntadas de más de 12 mm"),
            );
        }
        if !jef.trailing.is_empty() {
            ui.label(format!("⚠ {} bytes extra tras el fin", jef.trailing.len()));
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
            ui.colored_label(Color32::RED, "El diseño no cabe en este bastidor");
        }
    }

    /// Lista de bloques de color. Devuelve el bloque cuyo color se quiere cambiar, si alguno.
    fn color_section(ui: &mut egui::Ui, doc: &mut Doc) -> Option<usize> {
        ui.horizontal(|ui| {
            ui.strong("Colores");
            if ui.small_button("todos").clicked() {
                doc.selected.fill(true);
            }
            if ui.small_button("ninguno").clicked() {
                doc.selected.fill(false);
            }
        });
        ui.small("Clic: seleccionar · Ctrl+clic: agregar · nada seleccionado = todo");

        let mut pick = None;
        let mut action: Option<(usize, i8)> = None;
        let n = doc.blocks.len();
        for i in 0..n {
            let idx = doc.jef.colors.get(i).copied().unwrap_or(0);
            let name = palette::thread(idx).map_or("?", |t| t.name);
            let [r, g, b] = doc.jef.thread_rgb(i);
            ui.horizontal(|ui| {
                ui.checkbox(&mut doc.visible[i], "")
                    .on_hover_text("Mostrar/ocultar");
                let swatch = egui::Button::new("")
                    .fill(Color32::from_rgb(r, g, b))
                    .min_size(Vec2::new(20.0, 18.0));
                if ui.add(swatch).on_hover_text("Cambiar color").clicked() {
                    pick = Some(i);
                }
                let label = format!("{}. {name} ({idx})", i + 1);
                let response = ui
                    .selectable_label(doc.selected[i], label)
                    .on_hover_text(format!("{} entradas", doc.blocks[i].len()));
                if response.clicked() {
                    if ui.input(|inp| inp.modifiers.command) {
                        doc.selected[i] = !doc.selected[i];
                    } else {
                        let only_this = doc.selected[i] && doc.selection().len() == 1;
                        doc.selected.fill(false);
                        doc.selected[i] = !only_this;
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
                    .add_enabled(n > 1, egui::Button::new("🗑").small())
                    .on_hover_text("Borrar bloque")
                    .clicked()
                {
                    action = Some((i, 0));
                }
            });
        }
        match action {
            Some((i, 0)) => doc.remove_block(i),
            Some((i, d)) => doc.move_block(i, (i as isize + d as isize) as usize),
            None => {}
        }
        pick
    }

    fn transform_section(
        ui: &mut egui::Ui,
        doc: &mut Doc,
        step_mm: &mut f32,
        angle: &mut f64,
        scale_pct: &mut f64,
    ) {
        let sel = doc.selection();
        ui.strong(if sel.is_empty() {
            "Transformar (todo el diseño)"
        } else {
            "Transformar (selección)"
        });
        let step = (*step_mm * 10.0).round() as i32;
        ui.horizontal(|ui| {
            ui.label("Mover");
            for (label, dx, dy) in [("←", -1, 0), ("↑", 0, -1), ("↓", 0, 1), ("→", 1, 0)] {
                if ui.button(label).clicked() {
                    doc.edit(|j| j.translate(&sel, dx * step, dy * step));
                }
            }
            ui.add(
                egui::DragValue::new(step_mm)
                    .range(0.1..=50.0)
                    .speed(0.1)
                    .suffix(" mm"),
            );
        });
        ui.small("También: flechas del teclado, o Shift + arrastrar en el lienzo");
        ui.horizontal(|ui| {
            if ui.button("⟲ 90°").clicked() {
                doc.edit(|j| j.rotate(&sel, -90.0));
            }
            if ui.button("⟳ 90°").clicked() {
                doc.edit(|j| j.rotate(&sel, 90.0));
            }
            if ui.button("⇆ Espejo").clicked() {
                doc.edit(|j| j.mirror_x(&sel));
            }
            if ui.button("⇅ Espejo").clicked() {
                doc.edit(|j| j.mirror_y(&sel));
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(angle)
                    .range(-180.0..=180.0)
                    .speed(1.0)
                    .suffix("°"),
            );
            if ui.button("Rotar").clicked() {
                doc.edit(|j| j.rotate(&sel, *angle));
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(scale_pct)
                    .range(10.0..=500.0)
                    .speed(1.0)
                    .suffix(" %"),
            );
            if ui.button("Escalar").clicked() {
                doc.edit(|j| j.scale(&sel, *scale_pct / 100.0));
            }
        });
        if !jef::SAFE_SCALE.contains(&(*scale_pct / 100.0)) {
            ui.colored_label(
                Color32::from_rgb(200, 120, 0),
                "⚠ Fuera de 80–120 % la densidad del relleno cambia mucho",
            );
        }
        if ui.button("⌖ Centrar en el bastidor").clicked() {
            doc.edit(|j| j.center_on_hoop(&sel));
        }
    }

    /// Ventana para elegir un hilo de la paleta Janome.
    fn color_picker(&mut self, ctx: &egui::Context) {
        let (Some(block), Some(doc)) = (self.picking_color, &mut self.doc) else {
            return;
        };
        let current = doc.jef.colors.get(block).copied().unwrap_or(0);
        let mut open = true;
        let mut chosen = None;
        egui::Window::new(format!("Color del bloque {}", block + 1))
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
                                button = button.stroke(egui::Stroke::new(2.5, Color32::RED));
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
            if c != current {
                doc.edit(|j| j.set_color(block, c));
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
                    Some(Pending::Discard(path)) => self.open(&path),
                    Some(Pending::Overwrite(path)) => self.write_to(path),
                    None => {}
                }
            }
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match &self.doc {
            Some(doc) => format!(
                "{}{} — Editor JEF",
                if doc.dirty { "• " } else { "" },
                file_name(&doc.path)
            ),
            None => "Editor JEF".into(),
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
            self.request_open(path);
        }
        if self.pending.is_none() {
            self.shortcuts(ctx);
        }

        if self.playing
            && let Some(doc) = &self.doc
        {
            let total = doc.jef.stitches.len();
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
        self.update_title(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Panel::left("panel")
            .resizable(true)
            .default_size(320.0)
            .show(ui, |ui| self.side_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
        self.color_picker(&ctx);
        self.confirm_dialog(&ctx);
    }
}
