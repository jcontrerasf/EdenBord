//! Proyecto `.edenbord` abierto: objetos, selección, historial y las puntadas generadas.

use std::path::PathBuf;

use bordado::booleanas::{self, Booleana};
use bordado::geom::{self, Polilinea, polilineas};
use bordado::{Forma, Generado, Objeto, Proyecto, Puntada};
use jef::Bounds;
use kurbo::{Affine, Point, Rect};

use crate::nodos;

/// Cuántas versiones anteriores se guardan para deshacer.
const HISTORY_LIMIT: usize = 100;

pub struct ProyectoDoc {
    /// Archivo `.edenbord`; `None` si todavía no se guardó (proyecto nuevo o SVG importado).
    pub path: Option<PathBuf>,
    /// Nombre para mostrar y para sugerir al guardar.
    pub nombre: String,
    pub proyecto: Proyecto,
    pub seleccion: Vec<bool>,
    pub dirty: bool,
    /// Puntadas de los objetos visibles, recalculadas tras cada edición.
    pub generado: Generado,
    /// Formas de cada objeto como polilíneas (mm), para dibujar y elegir con el clic.
    pub lineas: Vec<Vec<Polilinea>>,
    /// Relleno de las guías que son zonas, en trapecios (mm), para verlas con sus agujeros.
    pub relleno_guias: Vec<Vec<[Point; 4]>>,
    /// Versiones anteriores y siguientes, cada una con su selección.
    undo: Vec<(Proyecto, Vec<bool>)>,
    redo: Vec<(Proyecto, Vec<bool>)>,
    /// Clave de la última edición agrupada (ver [`ProyectoDoc::edit_grouped`]).
    last_key: Option<String>,
}

impl ProyectoDoc {
    pub fn new(path: Option<PathBuf>, nombre: String, proyecto: Proyecto) -> Self {
        let generado = proyecto.generar();
        let mut doc = ProyectoDoc {
            path,
            nombre,
            proyecto,
            seleccion: vec![],
            dirty: false,
            generado,
            lineas: vec![],
            relleno_guias: vec![],
            undo: vec![],
            redo: vec![],
            last_key: None,
        };
        doc.sync();
        doc
    }

    /// Índices de los objetos seleccionados; vacío = todos.
    pub fn selection(&self) -> Vec<usize> {
        (0..self.seleccion.len())
            .filter(|&i| self.seleccion[i])
            .collect()
    }

    /// Guarda la versión actual en el historial antes de una edición.
    pub fn checkpoint(&mut self) {
        self.undo
            .push((self.proyecto.clone(), self.seleccion.clone()));
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        self.last_key = None;
    }

    /// Aplica una edición con su punto de deshacer.
    pub fn edit(&mut self, f: impl FnOnce(&mut Proyecto)) {
        self.checkpoint();
        f(&mut self.proyecto);
        self.sync();
    }

    /// Como [`edit`](Self::edit), pero ediciones seguidas con la misma clave (arrastrar un
    /// valor, escribir un texto) comparten un solo punto de deshacer.
    pub fn edit_grouped(&mut self, key: String, f: impl FnOnce(&mut Proyecto)) {
        if self.last_key.as_ref() != Some(&key) {
            self.checkpoint();
            self.last_key = Some(key);
        }
        f(&mut self.proyecto);
        self.sync();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Cierra el grupo de [`edit_grouped`](Self::edit_grouped) en curso: la próxima edición
    /// agrupada tendrá su propio punto de deshacer aunque use la misma clave.
    pub fn end_group(&mut self) {
        self.last_key = None;
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            let current = self.restore(prev);
            self.redo.push(current);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            let current = self.restore(next);
            self.undo.push(current);
        }
    }

    /// Pone una versión del historial y devuelve la actual.
    fn restore(&mut self, (proyecto, seleccion): (Proyecto, Vec<bool>)) -> (Proyecto, Vec<bool>) {
        let current = (
            std::mem::replace(&mut self.proyecto, proyecto),
            std::mem::replace(&mut self.seleccion, seleccion),
        );
        self.dirty = true;
        self.last_key = None;
        self.sync();
        current
    }

    /// Recalcula puntadas y formas, y ajusta la selección al número de objetos.
    pub fn sync(&mut self) {
        self.generado = self.proyecto.generar();
        self.lineas = self
            .proyecto
            .objetos
            .iter()
            .map(|o| o.trazado().map(|d| polilineas(&d)).unwrap_or_default())
            .collect();
        self.relleno_guias = self
            .proyecto
            .objetos
            .iter()
            .zip(&self.lineas)
            .map(|(o, pls)| {
                if o.puntada == Puntada::Ninguna && o.es_zona() {
                    trapecios(pls)
                } else {
                    vec![]
                }
            })
            .collect();
        self.seleccion.resize(self.proyecto.objetos.len(), false);
    }

    /// Aplica a la selección una transformación que depende de su rectángulo.
    pub fn transform(&mut self, sel: &[usize], f: impl FnOnce(Rect) -> Affine) {
        let Some(r) = self.rect_of(sel) else {
            return;
        };
        let t = f(r);
        self.edit(|p| {
            for (i, o) in p.objetos.iter_mut().enumerate() {
                if sel.is_empty() || sel.contains(&i) {
                    o.transformar(t);
                }
            }
        });
    }

    /// Aplica una transformación ya calculada (la de un gesto) a la selección.
    pub fn transform_affine(&mut self, sel: &[usize], t: Affine) {
        if t == Affine::IDENTITY {
            return;
        }
        self.transform(sel, |_| t);
    }

    /// Como [`transform_affine`](Self::transform_affine), pero las seguidas con la misma clave
    /// (arrastrar X, Y, ancho o alto) comparten un solo punto de deshacer.
    pub fn transform_grouped(&mut self, key: &str, sel: &[usize], t: Affine) {
        if t == Affine::IDENTITY {
            return;
        }
        self.edit_grouped(key.into(), |p| {
            for (i, o) in p.objetos.iter_mut().enumerate() {
                if sel.is_empty() || sel.contains(&i) {
                    o.transformar(t);
                }
            }
        });
    }

    /// Cambia la imagen de fondo con su punto de deshacer, sin regenerar puntadas (el fondo no
    /// se borda).
    pub fn edit_fondo(&mut self, f: impl FnOnce(&mut bordado::Fondo)) {
        if self.proyecto.fondo.is_some() {
            self.checkpoint();
            if let Some(fondo) = &mut self.proyecto.fondo {
                f(fondo);
            }
        }
    }

    /// Objetos que se pueden elegir en el lienzo (visibles y sin candado).
    pub fn elegible(&self, i: usize) -> bool {
        let o = &self.proyecto.objetos[i];
        o.visible && !o.bloqueado
    }

    /// Objetos elegibles cuyas formas caben enteras en `r` (mm).
    pub fn objects_in(&self, r: Rect) -> Vec<usize> {
        (0..self.proyecto.objetos.len())
            .filter(|&i| self.elegible(i))
            .filter(|&i| {
                self.rect_of(&[i])
                    .is_some_and(|o| crate::gestos::contiene(r, o))
            })
            .collect()
    }

    /// Rectángulo (mm) de las formas de `sel` (vacío = todas).
    pub fn rect_of(&self, sel: &[usize]) -> Option<Rect> {
        self.lineas
            .iter()
            .enumerate()
            .filter(|(i, _)| sel.is_empty() || sel.contains(i))
            .flat_map(|(_, pls)| pls.iter().flat_map(|pl| pl.puntos.iter()))
            .fold(None, |r: Option<Rect>, &p| {
                Some(r.map_or(Rect::from_points(p, p), |r| r.union_pt(p)))
            })
    }

    /// Rectángulo de la selección, solo si hay algo seleccionado.
    pub fn selection_rect(&self) -> Option<Rect> {
        let sel = self.selection();
        if sel.is_empty() {
            None
        } else {
            self.rect_of(&sel)
        }
    }

    /// Rectángulo en 0,1 mm de puntadas y formas, para ajustar la vista.
    pub fn bounds(&self) -> Bounds {
        let mut b = self.generado.jef.bounds();
        if let Some(r) = self.rect_of(&[]) {
            b.min_x = b.min_x.min((r.x0 * 10.0).floor() as i32);
            b.min_y = b.min_y.min((r.y0 * 10.0).floor() as i32);
            b.max_x = b.max_x.max((r.x1 * 10.0).ceil() as i32);
            b.max_y = b.max_y.max((r.y1 * 10.0).ceil() as i32);
        }
        b
    }

    /// Objeto bajo el punto (mm): el último en el orden que lo contenga (zonas) o que pase a
    /// menos de `tol` (bordes y líneas).
    ///
    /// Primero cuenta el borde (el de más arriba en el orden); si el punto no toca ningún
    /// borde, gana la zona más chica que lo contiene, así se puede elegir una forma que está
    /// dentro de otra más grande. Los objetos ocultos o bloqueados no se eligen.
    pub fn object_at(&self, p: Point, tol: f64) -> Option<usize> {
        let visibles = || {
            (0..self.proyecto.objetos.len())
                .rev()
                .filter(|&i| self.elegible(i))
        };
        let near = |i: usize| {
            self.lineas[i].iter().any(|pl| {
                let n = pl.puntos.len();
                let segs = if pl.cerrada { n } else { n - 1 };
                (0..segs).any(|k| {
                    let seg = kurbo::Line::new(pl.puntos[k], pl.puntos[(k + 1) % n]);
                    kurbo::ParamCurveNearest::nearest(&seg, p, 1e-6).distance_sq < tol * tol
                })
            })
        };
        visibles().find(|&i| near(i)).or_else(|| {
            visibles()
                .filter(|&i| {
                    self.proyecto.objetos[i].es_zona() && inside_even_odd(&self.lineas[i], p)
                })
                .min_by(|&a, &b| {
                    let area = |i| self.rect_of(&[i]).map_or(0.0, |r| r.area());
                    area(a).total_cmp(&area(b))
                })
        })
    }

    /// Agrega objetos al final y los deja seleccionados.
    pub fn add(&mut self, objetos: Vec<Objeto>) {
        let start = self.proyecto.objetos.len();
        self.edit(|p| p.objetos.extend(objetos));
        self.seleccion.fill(false);
        for s in &mut self.seleccion[start..] {
            *s = true;
        }
    }

    /// Mueve un objeto en el orden de bordado, junto con su selección.
    pub fn move_object(&mut self, from: usize, to: usize) {
        let n = self.proyecto.objetos.len();
        if from >= n || to >= n || from == to {
            return;
        }
        self.edit(|p| {
            let o = p.objetos.remove(from);
            p.objetos.insert(to, o);
        });
        let s = self.seleccion.remove(from);
        self.seleccion.insert(to, s);
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.proyecto.objetos.len() {
            self.seleccion.remove(index);
            self.edit(|p| {
                p.objetos.remove(index);
            });
        }
    }

    /// Ruta de la imagen de fondo; las relativas se toman desde la carpeta del proyecto.
    pub fn fondo_path(&self) -> Option<PathBuf> {
        let ruta = &self.proyecto.fondo.as_ref()?.ruta;
        Some(match self.path.as_deref().and_then(|p| p.parent()) {
            Some(dir) if ruta.is_relative() => dir.join(ruta),
            _ => ruta.clone(),
        })
    }

    /// Une los contornos seleccionados en uno solo (los de adentro quedan como agujeros).
    /// El resultado toma el lugar y las propiedades del primero. Devuelve si combinó.
    pub fn combine(&mut self) -> bool {
        let sel: Vec<usize> = self
            .selection()
            .into_iter()
            .filter(|&i| matches!(self.proyecto.objetos[i].forma, Forma::Contorno { .. }))
            .collect();
        let [first, rest @ ..] = sel.as_slice() else {
            return false;
        };
        if rest.is_empty() {
            return false;
        }
        let (first, rest) = (*first, rest.to_vec());
        self.edit(|p| {
            let mut extra = Vec::new();
            for &i in &rest {
                if let Forma::Contorno { d } = &p.objetos[i].forma {
                    extra.extend(d.elements().iter().copied());
                }
            }
            if let Forma::Contorno { d } = &mut p.objetos[first].forma {
                for el in extra {
                    d.push(el);
                }
            }
            let mut i = 0;
            p.objetos.retain(|_| {
                i += 1;
                !rest.contains(&(i - 1))
            });
        });
        self.seleccion = vec![false; self.proyecto.objetos.len()];
        self.seleccion[first] = true;
        true
    }

    /// Cuántos subtrazados tiene el objeto si es un contorno o una línea (0 si es texto).
    pub fn subpaths(&self, i: usize) -> usize {
        match &self.proyecto.objetos[i].forma {
            Forma::Contorno { d } | Forma::Linea { d } => geom::subtrazados(d).len(),
            Forma::Texto { .. } => 0,
        }
    }

    /// Separa cada contorno o línea seleccionado en un objeto por subtrazado, en su mismo lugar
    /// del orden de bordado (los agujeros pasan a ser zonas aparte). Deja seleccionadas las
    /// partes. Devuelve si separó algo.
    pub fn split(&mut self) -> bool {
        let sel: Vec<usize> = self
            .selection()
            .into_iter()
            .filter(|&i| self.subpaths(i) > 1)
            .collect();
        if sel.is_empty() {
            return false;
        }
        let mut seleccion = self.seleccion.clone();
        self.edit(|p| {
            for &i in sel.iter().rev() {
                let original = p.objetos.remove(i);
                let (Forma::Contorno { d } | Forma::Linea { d }) = &original.forma else {
                    unreachable!("solo se separan trazados");
                };
                let partes: Vec<Objeto> = geom::subtrazados(d)
                    .into_iter()
                    .enumerate()
                    .map(|(k, d)| Objeto {
                        nombre: format!("{} {}", original.nombre, k + 1),
                        forma: match original.forma {
                            Forma::Linea { .. } => Forma::Linea { d },
                            _ => Forma::Contorno { d },
                        },
                        ..original.clone()
                    })
                    .collect();
                let n = partes.len();
                p.objetos.splice(i..i, partes);
                seleccion.splice(i..=i, std::iter::repeat_n(true, n));
            }
        });
        self.seleccion = seleccion;
        true
    }

    /// Operación booleana entre los contornos seleccionados, en orden de bordado.
    /// Restar recorta el primero con los demás, que se conservan; las otras operaciones dejan
    /// el resultado en lugar del primero (con sus propiedades) y borran los demás.
    pub fn boolean(&mut self, op: Booleana) -> Result<(), &'static str> {
        let sel: Vec<usize> = self
            .selection()
            .into_iter()
            .filter(|&i| matches!(self.proyecto.objetos[i].forma, Forma::Contorno { .. }))
            .collect();
        if sel.len() < 2 {
            return Err(
                "Selecciona al menos dos zonas (los textos se convierten primero en trazado)",
            );
        }
        let zonas = sel.iter().map(|&i| match &self.proyecto.objetos[i].forma {
            Forma::Contorno { d } => d,
            _ => unreachable!("solo contornos"),
        });
        let d = booleanas::operar_varias(op, zonas).unwrap_or_default();
        if d.elements().is_empty() {
            return Err(match op {
                Booleana::Restar => {
                    "El resultado queda vacío: Restar recorta la zona que se borda primero \
                     (la de más arriba en la lista) con las demás, y esas la tapan entera. \
                     Arrastra el fondo al primer lugar de la lista"
                }
                Booleana::Intersecar => "El resultado queda vacío: las zonas no se tocan",
                _ => "El resultado queda vacío",
            });
        }
        let (first, rest) = (sel[0], &sel[1..]);
        self.edit(|p| {
            p.objetos[first].forma = Forma::Contorno { d };
            if op != Booleana::Restar {
                let mut i = 0;
                p.objetos.retain(|_| {
                    i += 1;
                    !rest.contains(&(i - 1))
                });
            }
        });
        self.seleccion = vec![false; self.proyecto.objetos.len()];
        self.seleccion[first] = true;
        Ok(())
    }

    /// Borra los objetos seleccionados (si no hay selección, no borra nada).
    pub fn remove_selected(&mut self) {
        let sel = self.selection();
        if sel.is_empty() {
            return;
        }
        self.edit(|p| {
            let mut i = 0;
            p.objetos.retain(|_| {
                i += 1;
                !sel.contains(&(i - 1))
            });
        });
        self.seleccion = vec![false; self.proyecto.objetos.len()];
    }

    /// Copia el objeto justo después del original y selecciona la copia.
    pub fn duplicate(&mut self, index: usize) {
        let Some(o) = self.proyecto.objetos.get(index).cloned() else {
            return;
        };
        self.edit(|p| {
            let mut o = o;
            o.nombre.push_str(" (copia)");
            p.objetos.insert(index + 1, o);
        });
        self.seleccion.insert(index + 1, false);
        self.seleccion.truncate(self.proyecto.objetos.len());
        self.seleccion.fill(false);
        self.seleccion[index + 1] = true;
    }

    /// Copias de los objetos seleccionados, en orden de bordado.
    pub fn selected_objects(&self) -> Vec<Objeto> {
        self.selection()
            .into_iter()
            .map(|i| self.proyecto.objetos[i].clone())
            .collect()
    }

    /// Agrega objetos pegados al final, seleccionados. Con `centro`, los mueve para que su
    /// rectángulo quede centrado ahí (mm); si no, quedan donde estaban.
    pub fn paste(&mut self, mut objetos: Vec<Objeto>, centro: Option<Point>) {
        if objetos.is_empty() {
            return;
        }
        if let (Some(c), Some(r)) = (centro, rect_objetos(&objetos)) {
            let t = Affine::translate(c - r.center());
            for o in &mut objetos {
                o.transformar(t);
            }
        }
        self.add(objetos);
    }

    /// Copia cada objeto seleccionado justo después del original, en el mismo lugar, y deja
    /// seleccionadas las copias.
    pub fn duplicate_selected(&mut self) {
        let sel = self.selection();
        if sel.is_empty() {
            return;
        }
        self.edit(|p| {
            for &i in sel.iter().rev() {
                let mut o = p.objetos[i].clone();
                o.nombre.push_str(" (copia)");
                p.objetos.insert(i + 1, o);
            }
        });
        // Original i queda en i + (copias antes de él); su copia, justo después.
        self.seleccion = vec![false; self.proyecto.objetos.len()];
        for (k, &i) in sel.iter().enumerate() {
            self.seleccion[i + k + 1] = true;
        }
    }

    /// El objeto cuyos nodos se pueden editar: el único seleccionado, si es contorno o línea.
    pub fn editable(&self) -> Option<usize> {
        match self.selection()[..] {
            [i] if matches!(
                self.proyecto.objetos[i].forma,
                Forma::Contorno { .. } | Forma::Linea { .. }
            ) =>
            {
                Some(i)
            }
            _ => None,
        }
    }

    /// Nodos del trazado del objeto `i` (si es contorno o línea).
    pub fn nodes(&self, i: usize) -> Option<Vec<nodos::Sub>> {
        match &self.proyecto.objetos.get(i)?.forma {
            Forma::Contorno { d } | Forma::Linea { d } => Some(nodos::desde_trazado(d)),
            Forma::Texto { .. } => None,
        }
    }

    /// Reemplaza el trazado del objeto `i` sin punto de deshacer ni regenerar puntadas (para
    /// arrastrar nodos: el arrastre guarda el punto al empezar y llama a `sync` al soltar).
    pub fn set_nodes_live(&mut self, i: usize, subs: &[nodos::Sub]) {
        let Some(o) = self.proyecto.objetos.get_mut(i) else {
            return;
        };
        if let Forma::Contorno { d } | Forma::Linea { d } = &mut o.forma {
            *d = nodos::a_trazado(subs);
            self.lineas[i] = polilineas(d);
            self.dirty = true;
        }
    }

    /// Edita los nodos del objeto `i` con un punto de deshacer; `f` devuelve si cambió algo.
    pub fn edit_nodes(&mut self, i: usize, f: impl FnOnce(&mut Vec<nodos::Sub>) -> bool) {
        let Some(mut subs) = self.nodes(i) else {
            return;
        };
        if f(&mut subs) {
            self.checkpoint();
            self.set_nodes_live(i, &subs);
            self.sync();
        }
    }

    /// Convierte un texto en contorno (deja de depender de la fuente; se pueden editar nodos).
    pub fn text_to_path(&mut self, i: usize) -> Result<(), String> {
        let d = self.proyecto.objetos[i].trazado()?;
        self.edit(|p| p.objetos[i].forma = Forma::Contorno { d });
        Ok(())
    }

    pub fn select_all(&mut self) {
        self.seleccion.fill(true);
    }
}

/// Rectángulo (mm) de las formas de unos objetos sueltos.
fn rect_objetos(objetos: &[Objeto]) -> Option<Rect> {
    objetos
        .iter()
        .filter_map(|o| o.trazado().ok())
        .flat_map(|d| polilineas(&d))
        .flat_map(|pl| pl.puntos)
        .fold(None, |r: Option<Rect>, p| {
            Some(r.map_or(Rect::from_points(p, p), |r| r.union_pt(p)))
        })
}

/// Regla par-impar sobre todas las polilíneas (abiertas se consideran cerradas).
fn inside_even_odd(pls: &[Polilinea], p: Point) -> bool {
    let mut inside = false;
    for pl in pls {
        let n = pl.puntos.len();
        for k in 0..n {
            let (a, b) = (pl.puntos[k], pl.puntos[(k + 1) % n]);
            if (a.y <= p.y) != (b.y <= p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Descompone la zona (regla par-impar; todas las polilíneas cuentan como cerradas) en
/// trapecios de bases horizontales: `[arriba-izq, arriba-der, abajo-der, abajo-izq]`.
pub fn trapecios(pls: &[Polilinea]) -> Vec<[Point; 4]> {
    // Lados no horizontales, de arriba hacia abajo, ordenados por donde empiezan.
    let mut lados: Vec<(Point, Point)> = pls
        .iter()
        .filter(|pl| pl.puntos.len() >= 3)
        .flat_map(|pl| {
            let n = pl.puntos.len();
            (0..n).map(move |k| (pl.puntos[k], pl.puntos[(k + 1) % n]))
        })
        .filter(|(a, b)| a.y != b.y)
        .map(|(a, b)| if a.y < b.y { (a, b) } else { (b, a) })
        .collect();
    lados.sort_by(|a, b| a.0.y.total_cmp(&b.0.y));
    let mut ys: Vec<f64> = lados.iter().flat_map(|(a, b)| [a.y, b.y]).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup();

    let x_en = |(a, b): (Point, Point), y: f64| a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y);
    let mut out = Vec::new();
    let mut activos: Vec<(Point, Point)> = Vec::new();
    let mut siguiente = 0;
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        while siguiente < lados.len() && lados[siguiente].0.y <= y0 {
            activos.push(lados[siguiente]);
            siguiente += 1;
        }
        activos.retain(|l| l.1.y > y0);
        let ym = (y0 + y1) / 2.0;
        let mut cruces: Vec<(f64, f64, f64)> = activos
            .iter()
            .map(|&l| (x_en(l, ym), x_en(l, y0), x_en(l, y1)))
            .collect();
        cruces.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &[a, b] in cruces.as_chunks::<2>().0 {
            out.push([
                Point::new(a.1, y0),
                Point::new(b.1, y0),
                Point::new(b.2, y1),
                Point::new(a.2, y1),
            ]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::BezPath;

    fn doc() -> ProyectoDoc {
        let obj = |d: &str, puntada: Puntada| Objeto {
            nombre: "x".into(),
            visible: true,
            color: 1,
            forma: Forma::Contorno {
                d: BezPath::from_svg(d).unwrap(),
            },
            puntada,
            opacidad: bordado::OPACIDAD_GUIA,
            bloqueado: false,
        };
        let mut p = Proyecto::default();
        // Cuadrado con agujero y, encima, un cuadrado chico dentro del agujero.
        p.objetos.push(obj(
            "M0,0 L30,0 L30,30 L0,30 Z M10,10 L20,10 L20,20 L10,20 Z",
            bordado::preset_por_nombre("Relleno 45°"),
        ));
        p.objetos
            .push(obj("M12,12 L14,12 L14,14 L12,14 Z", Puntada::Ninguna));
        ProyectoDoc::new(None, "prueba".into(), p)
    }

    #[test]
    fn picking_respects_holes_and_order() {
        let d = doc();
        assert_eq!(d.object_at(Point::new(5.0, 5.0), 0.1), Some(0));
        assert_eq!(d.object_at(Point::new(13.0, 13.0), 0.1), Some(1));
        assert_eq!(d.object_at(Point::new(17.0, 17.0), 0.1), None, "agujero");
        assert_eq!(d.object_at(Point::new(10.05, 17.0), 0.1), Some(0), "borde");
        assert_eq!(d.object_at(Point::new(40.0, 5.0), 0.1), None);
    }

    #[test]
    fn trapezoids_cover_zone_minus_holes() {
        let area = |ts: &[[Point; 4]]| -> f64 {
            ts.iter()
                .map(|t| ((t[1].x - t[0].x) + (t[2].x - t[3].x)) / 2.0 * (t[3].y - t[0].y))
                .sum()
        };
        let d = doc();
        assert!(
            (area(&trapecios(&d.lineas[0])) - 800.0).abs() < 1e-9,
            "30² − 10²"
        );
        let circulo = BezPath::from_svg("M10,0 A10,10 0 1 1 -10,0 A10,10 0 1 1 10,0 Z").unwrap();
        let a = area(&trapecios(&polilineas(&circulo)));
        assert!((a - std::f64::consts::PI * 100.0).abs() < 1.0, "{a}");
        // Solo las guías que son zonas se rellenan.
        assert!(d.relleno_guias[0].is_empty());
        assert!(!d.relleno_guias[1].is_empty());
    }

    #[test]
    fn picking_prefers_smaller_inner_shape_and_skips_hidden() {
        let mut d = doc();
        // Un disco grande encima de todo (sin agujero) tapa al cuadrado chico.
        d.edit(|p| {
            let mut o = p.objetos[1].clone();
            o.forma = Forma::Contorno {
                d: kurbo::BezPath::from_svg("M-5,-5 L40,-5 L40,40 L-5,40 Z").unwrap(),
            };
            p.objetos.push(o);
        });
        assert_eq!(
            d.object_at(Point::new(13.0, 13.0), 0.1),
            Some(1),
            "el más chico"
        );
        assert_eq!(d.object_at(Point::new(17.0, 17.0), 0.1), Some(2));
        assert_eq!(d.object_at(Point::new(10.05, 17.0), 0.1), Some(0), "borde");
        d.edit(|p| p.objetos[1].visible = false);
        assert_eq!(d.object_at(Point::new(13.0, 13.0), 0.1), Some(2), "oculto");
        assert_eq!(
            d.object_at(Point::new(12.0, 13.0), 0.1),
            Some(2),
            "ni su borde"
        );
    }

    #[test]
    fn edits_regenerate_and_undo() {
        let mut d = doc();
        let before = d.generado.jef.clone();
        d.transform(&[0], |r| Affine::translate(Point::ZERO - r.center()));
        assert_ne!(d.generado.jef, before);
        assert!(d.rect_of(&[0]).unwrap().center().distance(Point::ZERO) < 1e-9);
        d.undo();
        assert_eq!(d.generado.jef, before);

        d.edit_grouped("a".into(), |p| p.objetos[0].color = 2);
        d.edit_grouped("a".into(), |p| p.objetos[0].color = 3);
        d.undo();
        assert_eq!(
            d.proyecto.objetos[0].color, 1,
            "un solo deshacer para el grupo"
        );

        d.seleccion[1] = true;
        d.duplicate(1);
        assert_eq!(d.proyecto.objetos.len(), 3);
        assert_eq!(d.selection(), vec![2]);
        d.move_object(2, 0);
        assert_eq!(d.selection(), vec![0]);
        d.remove(0);
        assert_eq!(d.proyecto.objetos.len(), 2);
        assert!(d.selection().is_empty());
    }

    #[test]
    fn undo_restores_selection_and_groups_end() {
        let mut d = doc();
        d.seleccion[1] = true;
        d.remove_selected();
        assert_eq!(d.proyecto.objetos.len(), 1);
        d.undo();
        assert_eq!(d.selection(), vec![1], "lo borrado vuelve seleccionado");
        d.redo();
        assert!(d.selection().is_empty());

        d.edit_grouped("a".into(), |p| p.objetos[0].color = 2);
        d.end_group();
        d.edit_grouped("a".into(), |p| p.objetos[0].color = 3);
        d.undo();
        assert_eq!(d.proyecto.objetos[0].color, 2, "grupos separados");
    }

    #[test]
    fn box_selection_needs_whole_shape() {
        let d = doc();
        assert_eq!(d.objects_in(Rect::new(11.0, 11.0, 15.0, 15.0)), vec![1]);
        assert_eq!(d.objects_in(Rect::new(-1.0, -1.0, 31.0, 31.0)), vec![0, 1]);
        assert!(d.objects_in(Rect::new(-1.0, -1.0, 29.0, 31.0)).len() == 1);
    }

    #[test]
    fn duplicate_and_paste_select_the_new_objects() {
        let mut d = doc();
        d.select_all();
        d.duplicate_selected();
        assert_eq!(d.proyecto.objetos.len(), 4);
        assert_eq!(d.selection(), vec![1, 3]);
        assert_eq!(d.proyecto.objetos[1], {
            let mut o = d.proyecto.objetos[0].clone();
            o.nombre.push_str(" (copia)");
            o
        });

        let copia = d.selected_objects();
        d.paste(copia, Some(Point::new(100.0, 0.0)));
        assert_eq!(d.selection(), vec![4, 5]);
        let r = d.rect_of(&[4, 5]).unwrap();
        assert!(r.center().distance(Point::new(100.0, 0.0)) < 1e-9);
        d.undo();
        assert_eq!(d.proyecto.objetos.len(), 4);
    }

    #[test]
    fn combine_makes_holes() {
        let mut d = doc();
        // El cuadrado chico (objeto 1) pasa a ser agujero... dentro del agujero: vuelve a ser zona.
        d.seleccion.fill(true);
        assert!(d.combine());
        assert_eq!(d.proyecto.objetos.len(), 1);
        assert_eq!(d.selection(), vec![0]);
        assert_eq!(d.object_at(Point::new(17.0, 17.0), 0.1), None);
        assert_eq!(d.object_at(Point::new(13.0, 13.0), 0.1), Some(0));
        d.seleccion.fill(true);
        assert!(!d.combine(), "uno solo no se combina");
    }

    #[test]
    fn split_undoes_combine() {
        let mut d = doc();
        d.seleccion[0] = true;
        assert!(d.split());
        assert_eq!(d.proyecto.objetos.len(), 3);
        assert_eq!(
            d.selection(),
            vec![0, 1],
            "las dos partes, en el lugar del original"
        );
        assert_eq!(d.proyecto.objetos[0].nombre, "x 1");
        assert_eq!(d.proyecto.objetos[1].puntada, d.proyecto.objetos[0].puntada);
        // El borde exterior quedó lleno: ya no tiene agujero.
        assert_eq!(d.object_at(Point::new(17.0, 17.0), 0.1), Some(1));
        assert!(!d.split(), "ya no hay nada que separar");
        assert!(d.combine());
        assert_eq!(d.subpaths(0), 2);
        d.undo();
        d.undo();
        assert_eq!(d.proyecto.objetos.len(), 2);
    }

    #[test]
    fn subtract_keeps_the_pieces_on_top() {
        let mut d = doc();
        d.seleccion[0] = true;
        d.split();
        // Partes: 0 = cuadrado de 30 lleno, 1 = el ex agujero, 2 = el cuadrado chico.
        d.seleccion.fill(false);
        d.seleccion[0] = true;
        d.seleccion[2] = true;
        d.boolean(Booleana::Restar).unwrap();
        assert_eq!(d.proyecto.objetos.len(), 3);
        assert_eq!(d.selection(), vec![0]);
        assert_eq!(
            d.object_at(Point::new(13.0, 13.0), 0.1),
            Some(2),
            "hueco en el fondo"
        );

        d.seleccion.fill(true);
        d.boolean(Booleana::Unir).unwrap();
        assert_eq!(d.proyecto.objetos.len(), 1);
        d.seleccion.fill(false);
        d.seleccion[0] = true;
        assert!(d.boolean(Booleana::Unir).is_err(), "una sola zona");
    }
}
