//! Visor de archivos JEF.
//!
//! `cargo run -p visor -- [archivo.jef]`

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, Vec2};
use jef::{Jef, StitchKind, palette};

/// Puntadas por segundo de la simulación a velocidad 1×.
const PLAY_RATE: f32 = 400.0;

fn main() -> eframe::Result {
    let initial = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Visor JEF")
            .with_inner_size([1100.0, 750.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Visor JEF",
        options,
        Box::new(move |_cc| {
            let mut app = Visor::default();
            if let Some(path) = initial {
                app.open(&path);
            }
            Ok(Box::new(app))
        }),
    )
}

struct Loaded {
    path: PathBuf,
    jef: Jef,
    blocks: Vec<std::ops::Range<usize>>,
    visible: Vec<bool>,
}

struct Visor {
    loaded: Option<Loaded>,
    error: Option<String>,
    /// Escala en píxeles por 0,1 mm y desplazamiento del origen respecto al centro del lienzo.
    scale: f32,
    pan: Vec2,
    fit_pending: bool,
    show_jumps: bool,
    show_hoop: bool,
    fabric: Color32,
    /// Cuántas entradas de la secuencia se dibujan (simulación).
    progress: usize,
    playing: bool,
    speed: f32,
    play_accum: f32,
}

impl Default for Visor {
    fn default() -> Self {
        Self {
            loaded: None,
            error: None,
            scale: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            show_jumps: false,
            show_hoop: true,
            fabric: Color32::from_rgb(196, 190, 178),
            progress: 0,
            playing: false,
            speed: 1.0,
            play_accum: 0.0,
        }
    }
}

impl Visor {
    fn open(&mut self, path: &Path) {
        match jef::read_file(path) {
            Ok(jef) => {
                let blocks = jef.color_blocks();
                self.progress = jef.stitches.len();
                self.loaded = Some(Loaded {
                    path: path.to_path_buf(),
                    visible: vec![true; blocks.len()],
                    blocks,
                    jef,
                });
                self.error = None;
                self.playing = false;
                self.fit_pending = true;
            }
            Err(e) => self.error = Some(format!("No se pudo abrir {}: {e}", path.display())),
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        if ui.button("📂 Abrir…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Bordado Janome", &["jef", "JEF"])
                .pick_file()
        {
            self.open(&path);
        }
        if let Some(err) = &self.error {
            ui.colored_label(Color32::RED, err);
        }
        ui.separator();

        let Some(loaded) = &mut self.loaded else {
            ui.label("Abre un archivo .jef o arrástralo a la ventana.");
            return;
        };
        let jef = &loaded.jef;
        let name = loaded
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        ui.heading(name.as_ref());
        let (w, h) = jef.size_mm();
        egui::Grid::new("info").num_columns(2).show(ui, |ui| {
            ui.label("Tamaño");
            ui.label(format!("{w:.1} × {h:.1} mm"));
            ui.end_row();
            ui.label("Bastidor");
            ui.label(jef.header.hoop.name());
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
        if !jef.trailing.is_empty() {
            ui.label(format!("⚠ {} bytes extra tras el fin", jef.trailing.len()));
        }

        ui.separator();
        ui.label("Colores");
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                for (i, block) in loaded.blocks.iter().enumerate() {
                    let idx = jef.colors.get(i).copied().unwrap_or(0);
                    let name = palette::thread(idx).map_or("?", |t| t.name);
                    let [r, g, b] = jef.thread_rgb(i);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut loaded.visible[i], "");
                        egui::widgets::color_picker::show_color(
                            ui,
                            Color32::from_rgb(r, g, b),
                            Vec2::new(18.0, 18.0),
                        );
                        ui.label(format!("{}. {name} ({idx})", i + 1))
                            .on_hover_text(format!("{} entradas", block.len()));
                    });
                }
            });

        ui.separator();
        ui.checkbox(&mut self.show_jumps, "Mostrar saltos");
        ui.checkbox(&mut self.show_hoop, "Mostrar bastidor");
        ui.horizontal(|ui| {
            ui.label("Tela");
            ui.color_edit_button_srgba(&mut self.fabric);
        });
        if ui.button("Ajustar a la ventana").clicked() {
            self.fit_pending = true;
        }

        ui.separator();
        ui.label("Simulación");
        let total = jef.stitches.len();
        ui.horizontal(|ui| {
            let label = if self.playing {
                "⏸ Pausa"
            } else {
                "▶ Reproducir"
            };
            if ui.button(label).clicked() {
                if !self.playing && self.progress >= total {
                    self.progress = 0;
                }
                self.playing = !self.playing;
            }
            if ui.button("⏮").clicked() {
                self.progress = 0;
            }
            if ui.button("⏭").clicked() {
                self.progress = total;
                self.playing = false;
            }
        });
        ui.add(egui::Slider::new(&mut self.progress, 0..=total).text("puntada"));
        ui.add(
            egui::Slider::new(&mut self.speed, 0.25..=20.0)
                .logarithmic(true)
                .text("velocidad"),
        );
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, self.fabric);

        let Some(loaded) = &self.loaded else {
            return;
        };
        let jef = &loaded.jef;
        let bounds = jef.bounds();

        if self.fit_pending {
            let w = bounds.width().max(10) as f32;
            let h = bounds.height().max(10) as f32;
            self.scale = 0.9 * (rect.width() / w).min(rect.height() / h);
            let mid = Vec2::new(
                (bounds.min_x + bounds.max_x) as f32 / 2.0,
                (bounds.min_y + bounds.max_y) as f32 / 2.0,
            );
            self.pan = -mid * self.scale;
            self.fit_pending = false;
        }

        // Desplazar arrastrando; zoom con la rueda o el gesto, centrado en el puntero.
        self.pan += response.drag_delta();
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

        let origin = rect.center() + self.pan;
        let scale = self.scale;
        let to_screen = |x: i32, y: i32| origin + Vec2::new(x as f32, y as f32) * scale;

        if self.show_hoop
            && let Some((hw, hh)) = jef.header.hoop.size()
        {
            let hoop = Rect::from_two_pos(to_screen(-hw / 2, -hh / 2), to_screen(hw / 2, hh / 2));
            painter.rect_stroke(
                hoop,
                4.0,
                Stroke::new(1.5, Color32::from_gray(90)),
                egui::StrokeKind::Middle,
            );
            painter.line_segment(
                [to_screen(-30, 0), to_screen(30, 0)],
                Stroke::new(1.0, Color32::from_gray(90)),
            );
            painter.line_segment(
                [to_screen(0, -30), to_screen(0, 30)],
                Stroke::new(1.0, Color32::from_gray(90)),
            );
        }

        // Grosor aproximado de un hilo (0,4 mm), con un mínimo para que se vea al alejar.
        let width = (4.0 * scale).clamp(0.6, 6.0);
        let jump_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(40, 40, 40, 160));
        let mut shapes = Vec::new();
        for (b, block) in loaded.blocks.iter().enumerate() {
            if !loaded.visible[b] || block.start >= self.progress {
                continue;
            }
            let [r, g, bl] = jef.thread_rgb(b);
            let stroke = Stroke::new(width, Color32::from_rgb(r, g, bl));
            let end = block.end.min(self.progress);
            let mut run: Vec<Pos2> = Vec::new();
            for i in block.start..end {
                let s = jef.stitches[i];
                let p = to_screen(s.x, s.y);
                if s.kind == StitchKind::Normal {
                    if run.is_empty() && i > 0 {
                        let prev = jef.stitches[i - 1];
                        run.push(to_screen(prev.x, prev.y));
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

        // Marca de la aguja durante la simulación.
        if self.progress > 0 && self.progress < jef.stitches.len() {
            let s = jef.stitches[self.progress - 1];
            painter.circle_stroke(to_screen(s.x, s.y), 6.0, Stroke::new(2.0, Color32::RED));
        }
    }
}

impl eframe::App for Visor {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped {
            self.open(&path);
        }

        if self.playing
            && let Some(loaded) = &self.loaded
        {
            let total = loaded.jef.stitches.len();
            self.play_accum += ctx.input(|i| i.stable_dt) * PLAY_RATE * self.speed;
            let step = self.play_accum as usize;
            self.play_accum -= step as f32;
            self.progress = (self.progress + step).min(total);
            if self.progress >= total {
                self.playing = false;
            }
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("panel")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| self.side_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));
    }
}
