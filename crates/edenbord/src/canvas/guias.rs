//! Guides tool: draws and edits the angle guides and the cuts of an area satin. Dragging
//! across the shape adds a guide (or a cut: see [`Editor::guide_kind`], Ctrl swaps them);
//! dragging an end moves it along the edge; dragging the middle slides it; double click
//! straightens a guide; right click deletes it.
//!
//! Each column shows where it starts (a filled circle with its number in the sewing order)
//! and ends (an outlined one). Clicking an end makes the column start there; dragging a start
//! onto another column sews it in that place; clicking a start picks the column, to move it
//! from the context bar.

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
/// Radius of the circles of the sewing order, and how far (mm) into the column they go so the
/// end of one column and the start of the next don't cover each other.
const ORDER_PX: f32 = 8.0;
const ORDER_INSET: f64 = 1.5;
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
        let area = obj.fill && matches!(obj.puntada, Puntada::AreaSatin { .. });
        (area && !obj.bloqueado).then_some(i)
    }

    /// Where the circles of the sewing order go: start and end of each column, a little into
    /// it.
    fn order_marks(&self, obj: usize) -> Vec<(kurbo::Point, kurbo::Point)> {
        let doc = self.proyecto.as_ref().unwrap();
        doc.order[obj]
            .iter()
            .map(|&(s, e)| {
                let inset = ORDER_INSET.min(s.distance(e) / 4.0);
                let d = (e - s).normalize() * inset;
                if d.is_finite() {
                    (s + d, e - d)
                } else {
                    (s, e)
                }
            })
            .collect()
    }

    /// The circle of the sewing order under the pointer: column and whether it's its end.
    fn order_hit(&self, vista: Vista, pos: Pos2) -> Option<(usize, usize, bool)> {
        let obj = self.guide_object()?;
        self.order_marks(obj)
            .into_iter()
            .enumerate()
            .flat_map(|(k, (s, e))| [(k, false, s), (k, true, e)])
            .map(|(k, end, p)| (k, end, vista.mm(p).distance(pos)))
            .filter(|&(.., d)| d <= ORDER_PX + 2.0)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(k, end, _)| (obj, k, end))
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
        if self.order_input(response, vista) {
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

    /// Clicks and drags on the circles of the sewing order. `true` if it took the input.
    fn order_input(&mut self, response: &Response, vista: Vista) -> bool {
        use egui::PointerButton::Primary;
        if let Some(Gesto::Order { obj, column, to }) = &mut self.gesto {
            if let Some(pos) = response.interact_pointer_pos() {
                *to = vista.to_mm(pos);
            }
            let (obj, column, to) = (*obj, *column, *to);
            if response.drag_stopped() {
                self.gesto = None;
                let target = self.order_hit(vista, vista.mm(to)).map(|h| h.1);
                if let Some(j) = target.filter(|&j| j != column)
                    && let Some(doc) = self.proyecto.as_mut()
                {
                    doc.edit_order(obj, |starts| {
                        let s = starts.remove(column);
                        starts.insert(j, s);
                    });
                    self.selected_column = Some((obj, j));
                }
            }
            response.ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            return true;
        }
        let Some(pos) = response.hover_pos().or(response.interact_pointer_pos()) else {
            return false;
        };
        let press = response
            .ctx
            .input(|i| i.pointer.press_origin())
            .filter(|_| response.drag_started_by(Primary));
        if let Some(o) = press
            && let Some((obj, column, false)) = self.order_hit(vista, o)
        {
            self.selected_column = Some((obj, column));
            self.gesto = Some(Gesto::Order {
                obj,
                column,
                to: vista.to_mm(o),
            });
            return true;
        }
        let Some((obj, column, end)) = self.order_hit(vista, pos) else {
            return false;
        };
        if self.gesto.is_none() {
            response.ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked_by(Primary) {
            self.selected_guide = None;
            if end {
                // Start from this end instead.
                let doc = self.proyecto.as_mut().unwrap();
                let e = doc.order[obj][column].1;
                doc.edit_order(obj, |starts| starts[column] = e);
            }
            self.selected_column = Some((obj, column));
        }
        // Over a circle nothing else happens (no guide is drawn from it).
        response.clicked() || response.drag_started() || response.secondary_clicked()
    }

    /// The circles of the sewing order: a filled one with the number where each column starts,
    /// an outlined one where it ends; the picked column highlighted.
    fn draw_order(&self, painter: &egui::Painter, obj: usize, vista: Vista) {
        let picked = self.selected_column.filter(|c| c.0 == obj).map(|c| c.1);
        let font = egui::FontId::proportional(11.0);
        for (k, (s, e)) in self.order_marks(obj).into_iter().enumerate() {
            let color = if picked == Some(k) {
                tema::AVISO
            } else {
                tema::TEXTO
            };
            let (s, e) = (vista.mm(s), vista.mm(e));
            painter.line_segment([s, e], Stroke::new(1.0, color.gamma_multiply(0.5)));
            let number = (k + 1).to_string();
            painter.circle(e, ORDER_PX, Color32::WHITE, Stroke::new(1.5, color));
            painter.text(e, egui::Align2::CENTER_CENTER, &number, font.clone(), color);
            painter.circle(s, ORDER_PX, color, Stroke::new(1.5, Color32::WHITE));
            painter.text(
                s,
                egui::Align2::CENTER_CENTER,
                &number,
                font.clone(),
                Color32::WHITE,
            );
        }
        if let Some(Gesto::Order { obj: o, column, to }) = self.gesto
            && o == obj
            && let Some(&(s, _)) = self.order_marks(obj).get(column)
        {
            painter.line_segment([vista.mm(s), vista.mm(to)], Stroke::new(2.0, tema::AVISO));
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
        self.draw_order(painter, obj, vista);
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
