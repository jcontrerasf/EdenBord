//! El diseño abierto, su estado de interfaz por bloque y el historial de deshacer.

use std::ops::Range;
use std::path::PathBuf;

use jef::Jef;

/// Cuántas versiones anteriores se guardan para deshacer.
const HISTORY_LIMIT: usize = 100;

pub struct Doc {
    pub path: PathBuf,
    pub jef: Jef,
    /// Rangos de `jef.stitches` por bloque, recalculados tras cada edición.
    pub blocks: Vec<Range<usize>>,
    pub visible: Vec<bool>,
    pub selected: Vec<bool>,
    pub dirty: bool,
    undo: Vec<Jef>,
    redo: Vec<Jef>,
    /// Clave de la última edición agrupada (ver [`Doc::edit_grouped`]).
    last_key: Option<&'static str>,
}

impl Doc {
    pub fn new(path: PathBuf, jef: Jef) -> Self {
        let mut doc = Doc {
            path,
            jef,
            blocks: vec![],
            visible: vec![],
            selected: vec![],
            dirty: false,
            undo: vec![],
            redo: vec![],
            last_key: None,
        };
        doc.sync();
        doc
    }

    /// Índices de los bloques seleccionados; vacío = todos (así lo entiende `jef`).
    pub fn selection(&self) -> Vec<usize> {
        (0..self.selected.len())
            .filter(|&i| self.selected[i])
            .collect()
    }

    /// Guarda la versión actual en el historial antes de una edición.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.jef.clone());
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        self.last_key = None;
    }

    /// Aplica una edición con su punto de deshacer.
    pub fn edit(&mut self, f: impl FnOnce(&mut Jef)) {
        self.checkpoint();
        f(&mut self.jef);
        self.sync();
    }

    /// Como [`edit`](Self::edit), pero ediciones seguidas con la misma clave (arrastrar un
    /// valor) comparten un solo punto de deshacer.
    pub fn edit_grouped(&mut self, key: &'static str, f: impl FnOnce(&mut Jef)) {
        if self.last_key != Some(key) {
            self.checkpoint();
            self.last_key = Some(key);
        }
        f(&mut self.jef);
        self.sync();
    }

    /// Termina la edición agrupada en curso.
    pub fn end_group(&mut self) {
        self.last_key = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.jef, prev));
            self.dirty = true;
            self.sync();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.jef, next));
            self.dirty = true;
            self.sync();
        }
    }

    /// Mueve un bloque en el orden de bordado, junto con su visibilidad y selección.
    pub fn move_block(&mut self, from: usize, to: usize) {
        self.edit(|j| j.move_block(from, to));
        for flags in [&mut self.visible, &mut self.selected] {
            let f = flags.remove(from);
            flags.insert(to, f);
        }
    }

    pub fn remove_block(&mut self, index: usize) {
        if self.blocks.len() <= 1 {
            return;
        }
        self.visible.remove(index);
        self.selected.remove(index);
        self.edit(|j| {
            j.remove_block(index);
        });
    }

    /// Combina otro diseño y deja seleccionados sus bloques para ubicarlos.
    pub fn append(&mut self, other: &Jef) {
        let mut added = 0..0;
        self.edit(|j| added = j.append(other));
        self.selected.fill(false);
        for i in added {
            self.selected[i] = true;
        }
    }

    /// Recalcula los rangos y ajusta las banderas por bloque al nuevo número de bloques.
    pub fn sync(&mut self) {
        self.blocks = self.jef.color_blocks();
        let n = self.blocks.len();
        self.visible.resize(n, true);
        self.selected.resize(n, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jef::{Header, Hoop, Stitch, StitchKind};

    fn doc() -> Doc {
        let s = |x, y, kind| Stitch { x, y, kind };
        let jef = Jef {
            header: Header {
                flags: 0x0A,
                datetime: [0; 16],
                hoop: Hoop::A110x110,
                extents: [[0; 4]; 5],
                thread_types: vec![13; 3],
                gap: vec![],
            },
            colors: vec![1, 10, 3],
            stitches: vec![
                s(10, 0, StitchKind::Normal),
                s(10, 0, StitchKind::ColorChange),
                s(20, 0, StitchKind::Normal),
                s(20, 0, StitchKind::ColorChange),
                s(30, 0, StitchKind::Normal),
            ],
            trailing: vec![],
        };
        Doc::new(PathBuf::from("x.jef"), jef)
    }

    #[test]
    fn undo_redo_restore_versions() {
        let mut d = doc();
        let original = d.jef.clone();
        d.edit(|j| j.translate(&[], 5, 5));
        let moved = d.jef.clone();
        assert!(d.dirty && d.can_undo() && !d.can_redo());
        d.undo();
        assert_eq!(d.jef, original);
        d.redo();
        assert_eq!(d.jef, moved);
        d.undo();
        d.edit(|j| j.set_color(0, 2));
        assert!(!d.can_redo(), "una edición nueva borra el rehacer");
    }

    #[test]
    fn flags_follow_their_blocks() {
        let mut d = doc();
        d.visible[0] = false;
        d.selected[2] = true;
        d.move_block(0, 2);
        assert_eq!(d.jef.colors, vec![10, 3, 1]);
        assert_eq!(d.visible, vec![true, true, false]);
        assert_eq!(d.selection(), vec![1]);

        d.remove_block(1);
        assert_eq!(d.jef.colors, vec![10, 1]);
        assert_eq!(d.selection(), Vec::<usize>::new());

        let other = d.jef.clone();
        d.append(&other);
        assert_eq!(d.blocks.len(), 4);
        assert_eq!(d.selection(), vec![2, 3]);
        d.undo();
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.selected.len(), 2);
    }
}
