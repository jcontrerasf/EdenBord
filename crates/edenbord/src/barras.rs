//! Barras que rodean el lienzo: superior (archivo y edición), herramientas (columna
//! izquierda), estado (simulación, vista y mensajes) y la pantalla sin documento.

use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, RichText, Ui, Vec2};

use crate::{APP, Editor, Tool, iconos, open_dialog, tema};

impl Editor {
    pub(crate) fn top_bar(&mut self, ui: &mut Ui) {
        let proyecto = self.proyecto.is_some();
        // De derecha a izquierda: primero la acción principal y el menú, para que no se
        // tapen si la ventana es angosta; lo demás va en el espacio que queda.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.menu_button(tema::icono(iconos::MENU, false), |ui| {
                if proyecto && ui.button("Guardar como…").clicked() {
                    self.save_project(true);
                }
                ui.menu_button(format!("{} Recientes", iconos::RECIENTES), |ui| {
                    if self.recientes.is_empty() {
                        ui.weak("Nada todavía");
                    }
                    if let Some(path) = self.recent_list(ui, 0.0) {
                        self.request_open(Some(path));
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let zoom = ui.ctx().zoom_factor();
                    ui.label("Interfaz");
                    if tema::boton_icono(ui, iconos::ALEJAR, "Achicar (Ctrl −)").clicked() {
                        ui.ctx().set_zoom_factor((zoom - 0.1).max(0.5));
                    }
                    if ui
                        .button(format!("{:.0} %", zoom * 100.0))
                        .on_hover_text("Tamaño normal (Ctrl 0)")
                        .clicked()
                    {
                        ui.ctx().set_zoom_factor(1.0);
                    }
                    if tema::boton_icono(ui, iconos::ACERCAR, "Agrandar (Ctrl +)").clicked() {
                        ui.ctx().set_zoom_factor((zoom + 0.1).min(3.0));
                    }
                });
                ui.separator();
                if ui
                    .button(format!("{} Acerca de {APP}", iconos::INFO))
                    .clicked()
                {
                    self.about = true;
                }
            });
            if proyecto {
                if tema::boton_principal(ui, iconos::EXPORTAR, "Exportar JEF…")
                    .on_hover_text("Genera las puntadas para la máquina")
                    .clicked()
                {
                    self.export_jef();
                }
            } else if self.doc.is_some()
                && tema::boton_principal(ui, iconos::GUARDAR, "Guardar como…")
                    .on_hover_text("Ctrl+S")
                    .clicked()
            {
                self.save_as();
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.shrink_clip_rect(ui.max_rect());
                self.top_bar_left(ui, proyecto);
            });
        });
    }

    /// Logo, nombre y botones de archivo y edición.
    fn top_bar_left(&mut self, ui: &mut Ui, proyecto: bool) {
        ui.spacing_mut().item_spacing.x = 4.0;
        if let Some(logo) = &self.logo {
            ui.add(egui::Image::new(logo).fit_to_exact_size(Vec2::splat(24.0)))
                .on_hover_text(APP);
        }
        if let Some(name) = self.doc_name() {
            let dirty = if self.dirty() { " •" } else { "" };
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{name}{dirty}"))
                        .family(egui::FontFamily::Name(tema::FUERTE.into())),
                )
                .truncate(),
            );
        } else {
            ui.label(
                RichText::new(APP)
                    .family(egui::FontFamily::Name(tema::FUERTE.into()))
                    .color(tema::COPIHUE),
            );
        }
        ui.separator();
        if tema::boton_icono(ui, iconos::ABRIR, "Abrir… (.jef, .edenbord, .svg)").clicked()
            && let Some(path) = open_dialog().pick_file()
        {
            self.request_open(Some(path));
        }
        if tema::boton_icono(ui, iconos::NUEVO, "Proyecto nuevo").clicked() {
            self.request_open(None);
        }
        if proyecto && tema::boton_icono(ui, iconos::GUARDAR, "Guardar (Ctrl+S)").clicked() {
            self.save_project(false);
        }
        if self.doc.is_none() && !proyecto {
            return;
        }
        ui.separator();
        let (undo, redo) = self.can_undo_redo();
        if tema::boton_icono_si(ui, undo, iconos::DESHACER, "Deshacer (Ctrl+Z)").clicked() {
            self.undo();
        }
        if tema::boton_icono_si(ui, redo, iconos::REHACER, "Rehacer (Ctrl+Y)").clicked() {
            self.redo();
        }
        ui.separator();
        if proyecto {
            if tema::boton_icono(ui, iconos::SVG, "Importar SVG…").clicked() {
                self.import_svg();
            }
            if tema::boton_icono(ui, iconos::IMAGEN, "Imagen de fondo para calcar…").clicked() {
                self.pick_fondo();
            }
        } else if tema::boton_icono(ui, iconos::AGREGAR, "Agregar otro diseño JEF…").clicked() {
            self.add_design();
        }
    }

    pub(crate) fn tools_bar(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut tools = vec![(Tool::Select, iconos::SELECCIONAR, "Seleccionar (S)")];
            if self.proyecto.is_some() {
                tools.push((
                    Tool::Nodes,
                    iconos::NODOS,
                    "Nodos (N) · arrastrar nodos y manijas · doble clic en el trazo: agregar \
                     nodo · Supr: borrar nodo",
                ));
                tools.push((Tool::Pen, iconos::PLUMA, "Pluma (P) · calcar contornos"));
            }
            tools.push((
                Tool::Ruler,
                iconos::REGLA,
                "Regla (R) · arrastrar para medir · Shift: múltiplos de 45° · Esc: borrar",
            ));
            for (tool, ico, ayuda) in tools {
                if tema::interruptor(ui, self.tool == tool, ico, ayuda).clicked() {
                    self.set_tool(tool);
                }
            }
            if self.proyecto.is_some()
                && tema::boton_icono(ui, iconos::TEXTO, "Agregar texto…").clicked()
            {
                self.add_text();
            }
            ui.separator();
            if tema::interruptor(ui, self.show_grid, iconos::CUADRICULA, "Cuadrícula en mm")
                .clicked()
            {
                self.show_grid = !self.show_grid;
            }
            if tema::boton_icono(ui, iconos::AJUSTAR, "Ajustar a la ventana").clicked() {
                self.fit_pending = true;
            }
        });
    }

    /// Cambia de herramienta; la medida de la regla se borra al dejarla.
    pub(crate) fn set_tool(&mut self, tool: Tool) {
        if self.tool == Tool::Ruler && tool != Tool::Ruler {
            self.ruler = None;
        }
        self.tool = tool;
    }

    /// Simulación, opciones de vista, mensaje y posición del puntero.
    pub(crate) fn status_bar(&mut self, ui: &mut Ui) {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Ancho fijo: no salta al mover el puntero ni desaparece al salir del lienzo.
            let pos = self.pointer_mm.map_or(String::new(), |p| {
                format!("x {:6.1}  y {:6.1} mm", p.x, p.y)
            });
            ui.add_sized(
                [150.0, 20.0],
                egui::Label::new(RichText::new(pos).monospace().color(tema::TEXTO_SUAVE)),
            );
            ui.separator();
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.shrink_clip_rect(ui.max_rect());
                self.status_left(ui);
            });
        });
    }

    fn status_left(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.x = 4.0;
        let total = self.current_jef().map_or(0, |j| j.stitches.len());
        // `usize::MAX` = mostrar todo, aunque el diseño crezca al editarlo.
        let mut shown = self.progress.min(total);
        let (ico, ayuda) = if self.playing {
            (iconos::PAUSA, "Pausa")
        } else {
            (iconos::REPRODUCIR, "Simular el bordado")
        };
        if tema::interruptor(ui, self.playing, ico, ayuda).clicked() {
            if !self.playing && shown >= total {
                shown = 0;
            }
            self.playing = !self.playing;
        }
        if tema::boton_icono(ui, iconos::AL_INICIO, "Al inicio").clicked() {
            shown = 0;
        }
        if tema::boton_icono(ui, iconos::AL_FIN, "Todo").clicked() {
            shown = total;
            self.playing = false;
        }
        ui.spacing_mut().slider_width = (ui.available_width() - 420.0).clamp(60.0, 240.0);
        ui.add(egui::Slider::new(&mut shown, 0..=total).show_value(false))
            .on_hover_text(format!("{shown} de {total} puntadas"));
        self.progress = if shown >= total { usize::MAX } else { shown };
        ui.add(
            egui::DragValue::new(&mut self.speed)
                .range(0.25..=20.0)
                .speed(0.05)
                .max_decimals(2)
                .suffix("×"),
        )
        .on_hover_text("Velocidad de la simulación");
        ui.separator();

        let mut toggles = vec![
            (
                &mut self.realista,
                iconos::REALISTA,
                "Vista realista: hilos con volumen sobre la tela",
            ),
            (&mut self.show_jumps, iconos::SALTOS, "Mostrar saltos"),
            (&mut self.show_hoop, iconos::BASTIDOR, "Mostrar bastidor"),
        ];
        if self.proyecto.is_some() {
            toggles.push((
                &mut self.show_outlines,
                iconos::CONTORNOS,
                "Mostrar contornos de los objetos bordados",
            ));
        }
        for (flag, ico, ayuda) in toggles {
            if tema::interruptor(ui, *flag, ico, ayuda).clicked() {
                *flag = !*flag;
            }
        }
        ui.color_edit_button_srgba(&mut self.fabric)
            .on_hover_text("Color de la tela");
        if let Some((text, is_error)) = &self.message {
            ui.separator();
            let color = if *is_error {
                tema::COPIHUE
            } else {
                tema::TEXTO_SUAVE
            };
            ui.add(egui::Label::new(RichText::new(text).color(color)).truncate());
        }
    }

    /// Sin nada abierto: logo y botones para empezar.
    pub(crate) fn welcome(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let alto = 380.0 + 34.0 * self.recientes.len() as f32;
            ui.add_space(((ui.available_height() - alto) / 2.0).max(20.0));
            if let Some(logo) = &self.logo {
                ui.add(egui::Image::new(logo).fit_to_exact_size(Vec2::splat(128.0)));
            }
            ui.label(
                RichText::new(APP)
                    .size(32.0)
                    .family(egui::FontFamily::Name(tema::FUERTE.into()))
                    .color(tema::COPIHUE),
            );
            ui.label(
                RichText::new("«y tu campo de flores bordado es la copia feliz del Edén»")
                    .italics()
                    .color(tema::TEXTO_SUAVE),
            );
            ui.add_space(24.0);
            let ancho = 300.0;
            let grande = |ui: &mut Ui, ico: &str, texto: &str| {
                ui.add(
                    egui::Button::new(RichText::new(format!("{ico}   {texto}")).size(15.0))
                        .min_size(Vec2::new(ancho, 40.0)),
                )
            };
            if grande(ui, iconos::NUEVO, "Proyecto nuevo").clicked() {
                self.request_open(None);
            }
            if grande(ui, iconos::ABRIR, "Abrir…").clicked()
                && let Some(path) = open_dialog().pick_file()
            {
                self.request_open(Some(path));
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Abre un .jef, un proyecto .edenbord o un dibujo .svg, o arrástralo aquí.",
                )
                .color(tema::TEXTO_SUAVE),
            );
            if !self.recientes.is_empty() {
                ui.add_space(20.0);
                tema::titulo(ui, "Recientes");
                if let Some(path) = self.recent_list(ui, 300.0) {
                    self.request_open(Some(path));
                }
            }
            if let Some((text, true)) = &self.message {
                ui.colored_label(tema::COPIHUE, text);
            }
        });
    }

    /// Botones de los archivos recientes (los que ya no existen, deshabilitados). Devuelve el
    /// elegido. `ancho` 0 = el natural.
    fn recent_list(&self, ui: &mut Ui, ancho: f32) -> Option<PathBuf> {
        let mut elegido = None;
        for path in &self.recientes {
            let ico = match crate::extension(path).as_str() {
                "jef" => iconos::ARCHIVO_JEF,
                "svg" => iconos::SVG,
                "edenbord" | "bordado" => iconos::FLOR,
                _ => iconos::ARCHIVO,
            };
            let carpeta = path
                .parent()
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            let existe = path.exists();
            let boton = egui::Button::new(format!("{ico}  {}", crate::file_name(path)))
                .min_size(Vec2::new(ancho, 30.0));
            let resp = ui.add_enabled(existe, boton);
            let resp = if existe {
                resp.on_hover_text(carpeta)
            } else {
                resp.on_disabled_hover_text(format!("Ya no existe: {}", path.display()))
            };
            if resp.clicked() {
                elegido = Some(path.clone());
                ui.close();
            }
        }
        elegido
    }
}
