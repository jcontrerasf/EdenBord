//! Guides tool: draws and edits the angle guides of an area satin. Dragging across the shape
//! adds a guide; dragging an end moves it along the edge; dragging the middle slides the
//! whole guide; double click straightens it; right click deletes it.

use bordado::Puntada;
use bordado::puntadas::Guide;
use eframe::egui::{self, Color32, Pos2, Rect, Response, Shape, Stroke, Vec2};

use super::{ANGLE_COLOR, PICK_PX, Vista};
use crate::Editor;
use crate::gestos::{self, Gesto, GuideDrag};

/// Grab radius of a guide end, and distance to grab the line, in pixels.
const END_PX: f32 = 12.0;
const LINE_PX: f32 = 8.0;
/// Size of the squares at the ends.
const SQUARE_PX: f32 = 9.0;

/// What is under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GuideHit {
    pub obj: usize,
    pub guide: usize,
    /// `None` = on the line, not at an end.
    pub end: Option<usize>,
}

impl Editor {
    /// The angle guides being edited: those of the single area satin selected (not locked),
    /// with its index.
    pub(crate) fn guide_handles(&self) -> Option<(usize, &[Guide])> {
        let doc = self.proyecto.as_ref()?;
        let [i] = doc.selection()[..] else {
            return None;
        };
        let obj = &doc.proyecto.objetos[i];
        let area = matches!(obj.puntada, Puntada::AreaSatin { .. });
        (area && !obj.bloqueado).then(|| (i, &doc.guides[i][..]))
    }

    /// The guide under the pointer: ends first (closest), then lines.
    fn guide_hit(&self, vista: Vista, pos: Pos2) -> Option<GuideHit> {
        let (obj, guides) = self.guide_handles()?;
        let ends = guides
            .iter()
            .enumerate()
            .flat_map(|(k, g)| (0..2).map(move |e| (k, e, vista.mm(g[e]).distance(pos))));
        if let Some((guide, e, _)) = ends
            .filter(|&(_, _, d)| d <= END_PX)
            .min_by(|a, b| a.2.total_cmp(&b.2))
        {
            return Some(GuideHit {
                obj,
                guide,
                end: Some(e),
            });
        }
        guides
            .iter()
            .enumerate()
            .map(|(k, g)| (k, segment_distance(vista.mm(g[0]), vista.mm(g[1]), pos)))
            .filter(|&(_, d)| d <= LINE_PX)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(guide, _)| GuideHit {
                obj,
                guide,
                end: None,
            })
    }

    pub(super) fn guides_input(&mut self, ui: &egui::Ui, response: &Response, vista: Vista) {
        use egui::PointerButton::{Primary, Secondary};
        let (space, mods) = ui.input(|i| {
            let m = gestos::Mods {
                shift: i.modifiers.shift,
                ctrl: i.modifiers.command,
            };
            (i.key_down(egui::Key::Space), m)
        });
        if space {
            return;
        }
        if self.gesto.is_none()
            && let Some(pos) = response.hover_pos()
        {
            let icon = match self.guide_hit(vista, pos) {
                Some(GuideHit { end: Some(_), .. }) => egui::CursorIcon::Grab,
                Some(_) => egui::CursorIcon::Move,
                None if self.guide_handles().is_some() => egui::CursorIcon::Crosshair,
                None => egui::CursorIcon::Default,
            };
            ui.ctx().set_cursor_icon(icon);
        }

        if response.drag_started_by(Primary)
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            let desde = vista.to_mm(o);
            self.gesto = match self.guide_hit(vista, o) {
                Some(GuideHit { obj, guide, end }) => {
                    self.selected_guide = Some((obj, guide));
                    let drag = match end {
                        Some(e) => GuideDrag::End(e),
                        None => {
                            let g = self.proyecto.as_ref().unwrap().guides[obj][guide];
                            let v = g[1] - g[0];
                            GuideDrag::Slide {
                                angle: v.y.atan2(v.x),
                            }
                        }
                    };
                    Some(Gesto::Guide {
                        obj,
                        guide,
                        drag,
                        changed: false,
                    })
                }
                None => self.guide_handles().map(|(obj, _)| Gesto::NewGuide {
                    obj,
                    a: desde,
                    b: desde,
                }),
            };
        }
        if response.dragged_by(Primary)
            && let (Some(g), Some(pos)) = (&mut self.gesto, response.interact_pointer_pos())
        {
            let p = vista.to_mm(pos);
            g.arrastrar(p, mods);
            if let Gesto::Guide {
                obj,
                guide,
                drag,
                changed,
            } = g
                && let Some(doc) = self.proyecto.as_mut()
                && let Some(&old) = doc.guides[*obj].get(*guide)
            {
                let (obj, guide) = (*obj, *guide);
                let new = match *drag {
                    // The end follows the pointer along the edge of the shape.
                    GuideDrag::End(e) => doc.nearest_edge_point(obj, p).map(|q| {
                        let mut g = old;
                        g[e] = q;
                        g
                    }),
                    // The same slant through the pointer, cut to the edges again.
                    GuideDrag::Slide { angle } => {
                        let dir = kurbo::Vec2::new(angle.cos(), angle.sin());
                        doc.cut_across(obj, p - dir, p + dir)
                    }
                };
                if let Some(new) = new
                    && (new[0].distance(old[0]) > 1e-6 || new[1].distance(old[1]) > 1e-6)
                {
                    doc.edit_guides(obj, Some(format!("guia {obj}")), |g| g[guide] = new);
                    *changed = true;
                }
            }
        }
        if response.drag_stopped() {
            self.finish_gesture();
        }

        let hit = response
            .interact_pointer_pos()
            .and_then(|pos| self.guide_hit(vista, pos));
        // Click: pick a guide (Supr deletes it), or another object.
        if response.clicked_by(Primary) {
            self.selected_guide = hit.map(|h| (h.obj, h.guide));
            if hit.is_none()
                && let Some(pos) = response.interact_pointer_pos()
                && let Some(doc) = self.proyecto.as_mut()
            {
                let tol = f64::from(PICK_PX / (10.0 * vista.scale));
                if let Some(i) = doc.object_at(vista.to_mm(pos), tol) {
                    doc.seleccion.fill(false);
                    doc.seleccion[i] = true;
                }
            }
        }
        if let Some(h) = hit
            && let Some(doc) = self.proyecto.as_mut()
        {
            // Right click deletes the guide.
            if response.clicked_by(Secondary) {
                doc.remove_guide(h.obj, h.guide);
                self.selected_guide = None;
            }
            // Double click straightens it: across the column at its middle.
            if response.double_clicked_by(Primary) {
                let g = doc.guides[h.obj][h.guide];
                if let Some(new) = doc.guide_at(h.obj, g[0].midpoint(g[1])) {
                    doc.edit_guides(h.obj, None, |guides| guides[h.guide] = new);
                }
            }
        }
    }

    /// The guides of the area satin being edited, the one under the pointer highlighted, and
    /// the guide being drawn.
    pub(super) fn draw_guides(&self, painter: &egui::Painter, response: &Response, vista: Vista) {
        let Some((obj, guides)) = self.guide_handles() else {
            return;
        };
        let doc = self.proyecto.as_ref().unwrap();
        let auto = doc.proyecto.objetos[obj].guides.is_empty();
        let picked = self.selected_guide.filter(|s| s.0 == obj).map(|s| s.1);
        let hover = match self.gesto {
            Some(Gesto::Guide { guide, .. }) => Some(guide),
            Some(_) => None,
            None => response
                .hover_pos()
                .and_then(|pos| self.guide_hit(vista, pos))
                .map(|h| h.guide),
        };
        let halo = Stroke::new(4.0, Color32::from_white_alpha(170));
        for (k, g) in guides.iter().enumerate() {
            let (a, b) = (vista.mm(g[0]), vista.mm(g[1]));
            let strong = picked == Some(k) || hover == Some(k);
            let line = Stroke::new(if strong { 3.0 } else { 2.0 }, ANGLE_COLOR);
            painter.line_segment([a, b], halo);
            if auto && !strong {
                painter.extend(Shape::dashed_line(&[a, b], line, 6.0, 3.0));
            } else {
                painter.line_segment([a, b], line);
            }
            let fill = if picked == Some(k) {
                ANGLE_COLOR
            } else {
                Color32::WHITE
            };
            let size = if strong { SQUARE_PX + 2.0 } else { SQUARE_PX };
            for p in [a, b] {
                let q = Rect::from_center_size(p, Vec2::splat(size));
                painter.rect(
                    q,
                    0.0,
                    fill,
                    Stroke::new(1.5, ANGLE_COLOR),
                    egui::StrokeKind::Middle,
                );
            }
        }
        // The guide being drawn, and where it will be cut to the edges.
        if let Some(Gesto::NewGuide { obj, a, b }) = self.gesto {
            let (pa, pb) = (vista.mm(a), vista.mm(b));
            painter.extend(Shape::dashed_line(
                &[pa, pb],
                Stroke::new(1.5, ANGLE_COLOR),
                4.0,
                3.0,
            ));
            if let Some(g) = doc.cut_across(obj, a, b) {
                let (ga, gb) = (vista.mm(g[0]), vista.mm(g[1]));
                painter.line_segment([ga, gb], halo);
                painter.line_segment([ga, gb], Stroke::new(2.0, ANGLE_COLOR));
                for p in [ga, gb] {
                    painter.circle_filled(p, 3.5, ANGLE_COLOR);
                }
            }
        }
    }
}

fn segment_distance(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}
