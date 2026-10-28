//! Guides tool: draws and edits the angle guides and the cuts of an area satin. Dragging
//! across the shape adds a guide (or a cut: see [`Editor::guide_kind`], Ctrl swaps them);
//! dragging an end moves it along the edge; dragging the middle slides it; double click
//! straightens a guide; right click deletes it.

use bordado::Puntada;
use eframe::egui::{self, Color32, Pos2, Rect, Response, Shape, Stroke, Vec2};

use super::{ANGLE_COLOR, PICK_PX, Vista};
use crate::gestos::{self, Gesto, GuideDrag};
use crate::proyecto::LineKind;
use crate::{Editor, tema};

/// Grab radius of a guide end, and distance to grab the line, in pixels.
const END_PX: f32 = 12.0;
const LINE_PX: f32 = 8.0;
/// Size of the squares at the ends.
const SQUARE_PX: f32 = 9.0;
const KINDS: [LineKind; 2] = [LineKind::Cut, LineKind::Guide];

/// What is under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GuideHit {
    pub obj: usize,
    pub kind: LineKind,
    pub guide: usize,
    /// `None` = on the line, not at an end.
    pub end: Option<usize>,
}

impl Editor {
    /// The area satin whose guides and cuts are being edited: the single object selected, if
    /// it is one (and not locked).
    pub(crate) fn guide_object(&self) -> Option<usize> {
        let doc = self.proyecto.as_ref()?;
        let [i] = doc.selection()[..] else {
            return None;
        };
        let obj = &doc.proyecto.objetos[i];
        let area = matches!(obj.puntada, Puntada::AreaSatin { .. });
        (area && !obj.bloqueado).then_some(i)
    }

    /// The guide or cut under the pointer: ends first (closest), then lines.
    fn guide_hit(&self, vista: Vista, pos: Pos2) -> Option<GuideHit> {
        let obj = self.guide_object()?;
        let doc = self.proyecto.as_ref()?;
        let all = || {
            KINDS.into_iter().flat_map(move |kind| {
                doc.lines(obj, kind)
                    .iter()
                    .enumerate()
                    .map(move |(k, g)| (kind, k, (vista.mm(g[0]), vista.mm(g[1]))))
            })
        };
        let ends = all().flat_map(|(kind, k, (a, b))| {
            [(kind, k, 0, a.distance(pos)), (kind, k, 1, b.distance(pos))]
        });
        if let Some((kind, guide, e, _)) = ends
            .filter(|&(.., d)| d <= END_PX)
            .min_by(|a, b| a.3.total_cmp(&b.3))
        {
            return Some(GuideHit {
                obj,
                kind,
                guide,
                end: Some(e),
            });
        }
        all()
            .map(|(kind, k, (a, b))| (kind, k, segment_distance(a, b, pos)))
            .filter(|&(.., d)| d <= LINE_PX)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(kind, guide, _)| GuideHit {
                obj,
                kind,
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
                None if self.guide_object().is_some() => egui::CursorIcon::Crosshair,
                None => egui::CursorIcon::Default,
            };
            ui.ctx().set_cursor_icon(icon);
        }

        if response.drag_started_by(Primary)
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            let desde = vista.to_mm(o);
            self.gesto = match self.guide_hit(vista, o) {
                Some(GuideHit {
                    obj,
                    kind,
                    guide,
                    end,
                }) => {
                    self.selected_guide = Some((obj, kind, guide));
                    let drag = match end {
                        Some(e) => GuideDrag::End(e),
                        None => {
                            let g = self.proyecto.as_ref().unwrap().lines(obj, kind)[guide];
                            let v = g[1] - g[0];
                            GuideDrag::Slide {
                                angle: v.y.atan2(v.x),
                            }
                        }
                    };
                    Some(Gesto::Guide {
                        obj,
                        kind,
                        guide,
                        drag,
                        changed: false,
                    })
                }
                None => {
                    // Ctrl draws the other kind of line.
                    let kind = match (self.guide_kind, mods.ctrl) {
                        (LineKind::Guide, false) | (LineKind::Cut, true) => LineKind::Guide,
                        _ => LineKind::Cut,
                    };
                    self.guide_object().map(|obj| Gesto::NewGuide {
                        obj,
                        kind,
                        a: desde,
                        b: desde,
                    })
                }
            };
        }
        if response.dragged_by(Primary)
            && let (Some(g), Some(pos)) = (&mut self.gesto, response.interact_pointer_pos())
        {
            let p = vista.to_mm(pos);
            g.arrastrar(p, mods);
            if let Gesto::Guide {
                obj,
                kind,
                guide,
                drag,
                changed,
            } = g
                && let Some(doc) = self.proyecto.as_mut()
                && let Some(&old) = doc.lines(*obj, *kind).get(*guide)
            {
                let (obj, kind, guide) = (*obj, *kind, *guide);
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
                    let key = format!("guia {obj} {kind:?}");
                    doc.edit_lines(obj, kind, Some(key), |g| g[guide] = new);
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
            self.selected_guide = hit.map(|h| (h.obj, h.kind, h.guide));
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
            // Right click deletes it.
            if response.clicked_by(Secondary) {
                doc.remove_line(h.obj, h.kind, h.guide);
                self.selected_guide = None;
            }
            // Double click straightens a guide: across the column at its middle.
            if response.double_clicked_by(Primary) && h.kind == LineKind::Guide {
                let g = doc.guides[h.obj][h.guide];
                if let Some(new) = doc.guide_at(h.obj, g[0].midpoint(g[1])) {
                    doc.edit_guides(h.obj, None, |guides| guides[h.guide] = new);
                }
            }
        }
    }

    /// The guides and cuts of the area satin being edited, the one under the pointer
    /// highlighted, and the line being drawn. Guides have square ends, cuts round ones; both
    /// dashed while automatic.
    pub(super) fn draw_guides(&self, painter: &egui::Painter, response: &Response, vista: Vista) {
        let Some(obj) = self.guide_object() else {
            return;
        };
        let doc = self.proyecto.as_ref().unwrap();
        let o = &doc.proyecto.objetos[obj];
        let hover = match self.gesto {
            Some(Gesto::Guide { kind, guide, .. }) => Some((kind, guide)),
            Some(_) => None,
            None => response
                .hover_pos()
                .and_then(|pos| self.guide_hit(vista, pos))
                .map(|h| (h.kind, h.guide)),
        };
        let picked = self
            .selected_guide
            .filter(|s| s.0 == obj)
            .map(|s| (s.1, s.2));
        let halo = Stroke::new(4.0, Color32::from_white_alpha(170));
        for kind in KINDS {
            let (color, auto) = match kind {
                LineKind::Guide => (ANGLE_COLOR, o.guides.is_none()),
                LineKind::Cut => (tema::HOJA, o.cuts.is_none()),
            };
            for (k, g) in doc.lines(obj, kind).iter().enumerate() {
                let (a, b) = (vista.mm(g[0]), vista.mm(g[1]));
                let strong = picked == Some((kind, k)) || hover == Some((kind, k));
                let line = Stroke::new(if strong { 3.0 } else { 2.0 }, color);
                painter.line_segment([a, b], halo);
                if auto && !strong {
                    painter.extend(Shape::dashed_line(&[a, b], line, 6.0, 3.0));
                } else {
                    painter.line_segment([a, b], line);
                }
                let fill = if picked == Some((kind, k)) {
                    color
                } else {
                    Color32::WHITE
                };
                let size = if strong { SQUARE_PX + 2.0 } else { SQUARE_PX };
                for p in [a, b] {
                    let edge = Stroke::new(1.5, color);
                    match kind {
                        LineKind::Guide => {
                            painter.rect(
                                Rect::from_center_size(p, Vec2::splat(size)),
                                0.0,
                                fill,
                                edge,
                                egui::StrokeKind::Middle,
                            );
                        }
                        LineKind::Cut => {
                            painter.circle(p, size / 2.0, fill, edge);
                        }
                    }
                }
            }
        }
        // The line being drawn, and where it will be cut to the edges.
        if let Some(Gesto::NewGuide { obj, kind, a, b }) = self.gesto {
            let color = match kind {
                LineKind::Guide => ANGLE_COLOR,
                LineKind::Cut => tema::HOJA,
            };
            let (pa, pb) = (vista.mm(a), vista.mm(b));
            painter.extend(Shape::dashed_line(
                &[pa, pb],
                Stroke::new(1.5, color),
                4.0,
                3.0,
            ));
            if let Some(g) = doc.cut_across(obj, a, b) {
                let (ga, gb) = (vista.mm(g[0]), vista.mm(g[1]));
                painter.line_segment([ga, gb], halo);
                painter.line_segment([ga, gb], Stroke::new(2.0, color));
                for p in [ga, gb] {
                    painter.circle_filled(p, 3.5, color);
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
