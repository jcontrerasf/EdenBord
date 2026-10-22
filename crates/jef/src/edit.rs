//! Operaciones de edición.
//!
//! Las puntadas forman una sola secuencia, así que mover o reordenar un bloque de color por
//! separado haría que su primera puntada cosiera una línea desde donde terminó el bloque
//! anterior. Para evitarlo, todas las ediciones trabajan sobre [`Block`]s y luego se
//! reensamblan con [`Jef::set_blocks`], que conecta los bloques con cambio de color + salto.
//!
//! En las operaciones con `sel`, una selección vacía significa "todos los bloques".

use std::ops::RangeInclusive;

use crate::model::{Bounds, Jef, Stitch, StitchKind};

/// Rango de escala que no requiere re-densificar el relleno. Fuera de él, las puntadas quedan
/// demasiado largas o apretadas porque el archivo no guarda las formas originales.
pub const SAFE_SCALE: RangeInclusive<f64> = 0.8..=1.2;

/// Un bloque de color, independiente de los demás.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Índice en la paleta Janome.
    pub color: i32,
    /// Posición desde la que arranca el bloque: el origen para el primero y la posición del
    /// cambio de color para los demás. La primera puntada normal se cose desde aquí.
    pub anchor: (i32, i32),
    /// Puntadas y saltos del bloque, sin el cambio de color.
    pub stitches: Vec<Stitch>,
}

impl Block {
    /// Si la primera puntada se cose desde el anchor. Si no (empieza con un salto), el anchor
    /// es solo la posición donde quedó la aguja y no forma parte del dibujo.
    fn sews_from_anchor(&self) -> bool {
        self.stitches
            .first()
            .is_some_and(|s| s.kind == StitchKind::Normal)
    }

    /// Los puntos que una transformación debe mover: las puntadas y saltos del bloque, más el
    /// anchor solo si el bloque cose desde él. Así no se transforma el origen de la máquina
    /// ni el final del bloque anterior, y editar varias veces no acumula saltos.
    fn points_mut(&mut self) -> impl Iterator<Item = (&mut i32, &mut i32)> {
        let anchor = self
            .sews_from_anchor()
            .then_some((&mut self.anchor.0, &mut self.anchor.1));
        anchor
            .into_iter()
            .chain(self.stitches.iter_mut().map(|s| (&mut s.x, &mut s.y)))
    }

    /// Extremos de todas las puntadas cosidas del bloque.
    fn sewn_points(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let starts = std::iter::once(self.anchor).chain(self.stitches.iter().map(|s| (s.x, s.y)));
        starts
            .zip(&self.stitches)
            .filter(|(_, s)| s.kind == StitchKind::Normal)
            .flat_map(|(from, s)| [from, (s.x, s.y)])
    }
}

impl Bounds {
    /// Rectángulo de un conjunto de puntos, o `None` si está vacío.
    pub fn of(points: impl IntoIterator<Item = (i32, i32)>) -> Option<Bounds> {
        points.into_iter().fold(None, |b, (x, y)| {
            Some(match b {
                None => Bounds {
                    min_x: x,
                    min_y: y,
                    max_x: x,
                    max_y: y,
                },
                Some(b) => Bounds {
                    min_x: b.min_x.min(x),
                    min_y: b.min_y.min(y),
                    max_x: b.max_x.max(x),
                    max_y: b.max_y.max(y),
                },
            })
        })
    }
}

impl Jef {
    /// Separa la secuencia en bloques de color.
    pub fn blocks(&self) -> Vec<Block> {
        self.color_blocks()
            .into_iter()
            .enumerate()
            .map(|(i, range)| {
                let slice = &self.stitches[range];
                let (anchor, stitches) = match slice.split_first() {
                    Some((cc, rest)) if i > 0 => ((cc.x, cc.y), rest),
                    _ => ((0, 0), slice),
                };
                Block {
                    color: self.colors.get(i).copied().unwrap_or(1),
                    anchor,
                    stitches: stitches.to_vec(),
                }
            })
            .collect()
    }

    /// Reemplaza la secuencia por estos bloques, conectándolos con cambio de color y, si el
    /// bloque cose desde su anchor y la aguja no está ahí, un salto hasta él. No toca la cabecera.
    pub fn set_blocks(&mut self, blocks: Vec<Block>) {
        let mut stitches = Vec::new();
        let mut colors = Vec::with_capacity(blocks.len());
        let mut pos = (0, 0);
        for (k, block) in blocks.into_iter().enumerate() {
            if k > 0 {
                stitches.push(Stitch {
                    x: pos.0,
                    y: pos.1,
                    kind: StitchKind::ColorChange,
                });
            }
            // Si el bloque empieza saltando, su anchor no importa: no hace falta ir hasta ahí.
            if pos != block.anchor && block.sews_from_anchor() {
                stitches.push(Stitch {
                    x: block.anchor.0,
                    y: block.anchor.1,
                    kind: StitchKind::Jump,
                });
            }
            pos = block.stitches.last().map_or(block.anchor, |s| (s.x, s.y));
            stitches.extend(block.stitches);
            colors.push(block.color);
        }
        self.stitches = stitches;
        self.colors = colors;
    }

    /// Aplica `f` a los bloques seleccionados y reensambla.
    fn edit_selected(&mut self, sel: &[usize], mut f: impl FnMut(&mut [&mut Block])) {
        let mut blocks = self.blocks();
        let mut chosen: Vec<&mut Block> = blocks
            .iter_mut()
            .enumerate()
            .filter(|(i, _)| sel.is_empty() || sel.contains(i))
            .map(|(_, b)| b)
            .collect();
        f(&mut chosen);
        self.set_blocks(blocks);
        self.refresh_header();
    }

    /// Rectángulo de lo que se cose en los bloques seleccionados (sin saltos).
    pub fn selection_bounds(&self, sel: &[usize]) -> Option<Bounds> {
        let blocks = self.blocks();
        let points = blocks
            .iter()
            .enumerate()
            .filter(|(i, _)| sel.is_empty() || sel.contains(i))
            .flat_map(|(_, b)| b.sewn_points());
        Bounds::of(points)
    }

    /// Aplica una transformación de puntos alrededor del centro de la selección.
    /// `f` recibe la suma `min + max` de cada eje (el doble del centro, para que espejar sea
    /// exacto) y el punto.
    fn transform(&mut self, sel: &[usize], f: impl Fn((i32, i32), (i32, i32)) -> (i32, i32)) {
        let Some(b) = self.selection_bounds(sel) else {
            return;
        };
        let sum = (b.min_x + b.max_x, b.min_y + b.max_y);
        self.edit_selected(sel, |blocks| {
            for block in blocks.iter_mut() {
                for (x, y) in block.points_mut() {
                    (*x, *y) = f(sum, (*x, *y));
                }
            }
        });
    }

    /// Mueve la selección, en 0,1 mm.
    pub fn translate(&mut self, sel: &[usize], dx: i32, dy: i32) {
        self.edit_selected(sel, |blocks| {
            for block in blocks.iter_mut() {
                for (x, y) in block.points_mut() {
                    *x += dx;
                    *y += dy;
                }
            }
        });
    }

    /// Gira la selección en sentido horario (en pantalla) alrededor de su centro.
    /// Los múltiplos de 90° son exactos; los demás ángulos redondean a 0,1 mm.
    pub fn rotate(&mut self, sel: &[usize], degrees: f64) {
        let quarter = degrees / 90.0;
        if quarter.fract() == 0.0 {
            let turns = (quarter as i64).rem_euclid(4);
            self.transform(sel, |sum, (x, y)| {
                let (cx, cy) = (sum.0.div_euclid(2), sum.1.div_euclid(2));
                let (rx, ry) = (x - cx, y - cy);
                let (rx, ry) = match turns {
                    1 => (-ry, rx),
                    2 => (-rx, -ry),
                    3 => (ry, -rx),
                    _ => (rx, ry),
                };
                (cx + rx, cy + ry)
            });
        } else {
            let (sin, cos) = degrees.to_radians().sin_cos();
            self.transform(sel, |sum, (x, y)| {
                let (cx, cy) = (f64::from(sum.0) / 2.0, f64::from(sum.1) / 2.0);
                let (rx, ry) = (f64::from(x) - cx, f64::from(y) - cy);
                (
                    (cx + rx * cos - ry * sin).round() as i32,
                    (cy + rx * sin + ry * cos).round() as i32,
                )
            });
        }
    }

    /// Espejo horizontal (izquierda ↔ derecha).
    pub fn mirror_x(&mut self, sel: &[usize]) {
        self.transform(sel, |sum, (x, y)| (sum.0 - x, y));
    }

    /// Espejo vertical (arriba ↔ abajo).
    pub fn mirror_y(&mut self, sel: &[usize]) {
        self.transform(sel, |sum, (x, y)| (x, sum.1 - y));
    }

    /// Escala la selección alrededor de su centro. Ver [`SAFE_SCALE`].
    pub fn scale(&mut self, sel: &[usize], factor: f64) {
        self.transform(sel, |sum, (x, y)| {
            let (cx, cy) = (f64::from(sum.0) / 2.0, f64::from(sum.1) / 2.0);
            (
                (cx + (f64::from(x) - cx) * factor).round() as i32,
                (cy + (f64::from(y) - cy) * factor).round() as i32,
            )
        });
    }

    /// Mueve la selección para que su centro quede en el centro del bastidor.
    pub fn center_on_hoop(&mut self, sel: &[usize]) {
        if let Some(b) = self.selection_bounds(sel) {
            let (cx, cy) = ((b.min_x + b.max_x) / 2, (b.min_y + b.max_y) / 2);
            self.translate(sel, -cx, -cy);
        }
    }

    pub fn set_color(&mut self, block: usize, color: i32) {
        if let Some(c) = self.colors.get_mut(block) {
            *c = color;
        }
    }

    /// Mueve el bloque `from` a la posición `to` en el orden de bordado.
    pub fn move_block(&mut self, from: usize, to: usize) {
        let mut blocks = self.blocks();
        if from >= blocks.len() || to >= blocks.len() || from == to {
            return;
        }
        let block = blocks.remove(from);
        blocks.insert(to, block);
        self.set_blocks(blocks);
        self.refresh_header();
    }

    /// Borra un bloque. No borra el último que queda; devuelve si borró.
    pub fn remove_block(&mut self, index: usize) -> bool {
        let mut blocks = self.blocks();
        if blocks.len() <= 1 || index >= blocks.len() {
            return false;
        }
        blocks.remove(index);
        self.set_blocks(blocks);
        self.refresh_header();
        true
    }

    /// Agrega los bloques de `other` al final (su origen queda en el centro del bastidor).
    /// Devuelve los índices de los bloques nuevos.
    pub fn append(&mut self, other: &Jef) -> std::ops::Range<usize> {
        let mut blocks = self.blocks();
        let start = blocks.len();
        blocks.extend(other.blocks());
        let end = blocks.len();
        self.set_blocks(blocks);
        self.refresh_header();
        start..end
    }

    /// Cuenta las puntadas normales más largas que `max` o más cortas que `min` (0,1 mm).
    /// Útil después de escalar: el escritor parte las largas, pero el resultado se ve mal.
    pub fn odd_stitches(&self, min: i32, max: i32) -> (usize, usize) {
        let (mut short, mut long) = (0, 0);
        let mut prev = (0, 0);
        for s in &self.stitches {
            if s.kind == StitchKind::Normal {
                let len = f64::from(s.x - prev.0).hypot(f64::from(s.y - prev.1));
                if len > f64::from(max) {
                    long += 1;
                } else if len < f64::from(min) {
                    short += 1;
                }
            }
            prev = (s.x, s.y);
        }
        (short, long)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Header, Hoop, read, write};
    use StitchKind::*;

    /// Dos bloques: un cuadrado negro y una línea roja separada por un salto.
    fn sample() -> Jef {
        let s = |x, y, kind| Stitch { x, y, kind };
        let mut jef = Jef {
            header: Header {
                flags: 0x0A,
                datetime: [0; 16],
                hoop: Hoop::A110x110,
                extents: [[0; 4]; 5],
                thread_types: vec![],
                gap: vec![],
            },
            colors: vec![1, 10, 3],
            stitches: vec![
                s(-50, -50, Jump),
                s(50, -50, Normal),
                s(50, 50, Normal),
                s(-50, 50, Normal),
                s(-50, -50, Normal),
                s(-50, -50, ColorChange),
                s(100, 0, Jump),
                s(200, 0, Normal),
                s(300, 30, Normal),
                s(300, 30, ColorChange),
                s(310, 30, Normal),
            ],
            trailing: vec![],
        };
        jef.refresh_header();
        jef
    }

    /// Una puntada cosida: color, desde, hasta.
    type Segment = (i32, (i32, i32), (i32, i32));

    /// Segmentos cosidos (desde, hasta) agrupados por color, ordenados.
    fn sewn(jef: &Jef) -> Vec<Segment> {
        let mut out = Vec::new();
        let mut block = 0;
        let mut prev = (0, 0);
        for s in &jef.stitches {
            match s.kind {
                ColorChange => block += 1,
                Normal => out.push((jef.colors[block], prev, (s.x, s.y))),
                Jump => {}
            }
            prev = (s.x, s.y);
        }
        out.sort();
        out
    }

    /// Escribir y releer conserva lo cosido (salvo que haya puntadas que el escritor deba
    /// partir) y escribir de nuevo da los mismos bytes.
    fn assert_writable(jef: &Jef) {
        let bytes = write(jef);
        let back = read(&bytes).unwrap();
        let needs_split = sewn(jef).iter().any(|(_, a, b)| {
            (b.0 - a.0).abs() > crate::MAX_DELTA || (b.1 - a.1).abs() > crate::MAX_DELTA
        });
        if !needs_split {
            assert_eq!(sewn(&back), sewn(jef));
        }
        assert_eq!(back.colors, jef.colors);
        assert_eq!(write(&back), bytes);
    }

    #[test]
    fn blocks_roundtrip_is_identity() {
        let jef = sample();
        let mut again = jef.clone();
        again.set_blocks(jef.blocks());
        assert_eq!(again, jef);
    }

    #[test]
    fn quarter_turns_and_mirrors_return_to_start() {
        let jef = sample();
        let mut r = jef.clone();
        for _ in 0..4 {
            r.rotate(&[], 90.0);
        }
        assert_eq!(sewn(&r), sewn(&jef));

        let mut m = jef.clone();
        m.mirror_x(&[]);
        assert_ne!(sewn(&m), sewn(&jef));
        m.mirror_x(&[]);
        m.mirror_y(&[]);
        m.mirror_y(&[]);
        assert_eq!(sewn(&m), sewn(&jef));

        let mut t = jef.clone();
        t.translate(&[1], 37, -12);
        t.translate(&[1], -37, 12);
        assert_eq!(sewn(&t), sewn(&jef));
        assert_writable(&t);
    }

    #[test]
    fn repeated_edits_do_not_accumulate_jumps() {
        let mut jef = sample();
        jef.rotate(&[], 90.0);
        jef.translate(&[1], 10, 10);
        let jumps = jef.count(Jump);
        for _ in 0..8 {
            jef.rotate(&[], 90.0);
            jef.translate(&[1], 10, 10);
            jef.mirror_x(&[2]);
        }
        assert_eq!(jef.count(Jump), jumps);
    }

    #[test]
    fn rotate_90_maps_right_to_down() {
        let mut jef = sample();
        jef.rotate(&[1], 90.0);
        let b = jef.selection_bounds(&[1]).unwrap();
        // La línea roja era ancha (100..300) y baja (0..30): ahora es angosta y alta.
        assert!(b.height() > b.width());
        assert_writable(&jef);
    }

    #[test]
    fn moving_one_block_never_sews_across() {
        let mut jef = sample();
        jef.translate(&[1], 0, 400);
        let red: Vec<_> = sewn(&jef).into_iter().filter(|s| s.0 == 10).collect();
        assert_eq!(
            red,
            vec![(10, (100, 400), (200, 400)), (10, (200, 400), (300, 430))]
        );
        // El bloque siguiente arranca desde su anchor original, con un salto.
        assert!(sewn(&jef).contains(&(3, (300, 30), (310, 30))));
        assert_writable(&jef);
    }

    #[test]
    fn reorder_remove_and_append_keep_sewn_segments() {
        let jef = sample();
        let mut r = jef.clone();
        r.move_block(0, 2);
        assert_eq!(r.colors, vec![10, 3, 1]);
        assert_eq!(sewn(&r), sewn(&jef));
        assert_eq!(
            r.stitches[0].kind, Jump,
            "el primer bloque nuevo arranca saltando"
        );
        assert_writable(&r);

        let mut d = jef.clone();
        assert!(d.remove_block(1));
        assert_eq!(d.colors, vec![1, 3]);
        let expected: Vec<_> = sewn(&jef).into_iter().filter(|s| s.0 != 10).collect();
        assert_eq!(sewn(&d), expected);
        assert_writable(&d);
        assert!(d.remove_block(0));
        assert!(!d.remove_block(0), "no se borra el último bloque");

        let mut a = jef.clone();
        let other = sample();
        assert_eq!(a.append(&other), 3..6);
        assert_eq!(a.colors.len(), 6);
        assert_eq!(sewn(&a).len(), 2 * sewn(&jef).len());
        assert_writable(&a);
    }

    #[test]
    fn scale_and_center() {
        let mut jef = sample();
        jef.scale(&[0], 2.0);
        let b = jef.selection_bounds(&[0]).unwrap();
        assert_eq!((b.width(), b.height()), (200, 200));
        assert_eq!(jef.odd_stitches(5, 120).1, 4, "lados de 20 mm");

        jef.center_on_hoop(&[]);
        let b = jef.selection_bounds(&[]).unwrap();
        assert!((b.min_x + b.max_x).abs() <= 1 && (b.min_y + b.max_y).abs() <= 1);
        assert_writable(&jef);
    }
}
