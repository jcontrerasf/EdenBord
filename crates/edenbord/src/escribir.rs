//! Herramienta Texto: clic en el lienzo crea un texto ahí y se escribe directamente (las letras
//! se ven bordadas mientras se escribe); clic en un texto existente lo sigue editando.

use bordado::fuentes;
use bordado::{Forma, Objeto, preset_por_nombre};
use eframe::egui::{self, Event, Key};
use kurbo::{Affine, Point};

use crate::Editor;

/// Texto en edición en el lienzo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Escritura {
    /// Objeto que se escribe.
    pub obj: usize,
    /// Largo del historial al crearlo, si es nuevo: si queda vacío se descarta sin rastro.
    pub nuevo: Option<usize>,
    /// Posición del cursor, en caracteres desde el principio.
    pub cursor: usize,
}

/// Lo que piden las teclas mientras se escribe.
#[derive(Debug, Clone, PartialEq)]
enum Tecla {
    Escribir(String),
    /// Retroceso: borra antes del cursor.
    Borrar,
    /// Supr: borra después del cursor.
    Suprimir,
    Izquierda,
    Derecha,
    Arriba,
    Abajo,
    Inicio,
    Fin,
    Terminar,
}

/// Índice en bytes del carácter número `c` (o el largo, si no hay tantos).
fn byte(texto: &str, c: usize) -> usize {
    texto.char_indices().nth(c).map_or(texto.len(), |(b, _)| b)
}

/// Línea y columna (en caracteres) del cursor.
fn linea_columna(texto: &str, cursor: usize) -> (usize, usize) {
    let antes: String = texto.chars().take(cursor).collect();
    let linea = antes.matches('\n').count();
    let columna = antes.chars().rev().take_while(|&c| c != '\n').count();
    (linea, columna)
}

/// Cursor en la línea `linea`, columna `columna` (limitada al largo de la línea).
fn en_linea(texto: &str, linea: usize, columna: usize) -> usize {
    let mut cursor = 0;
    for (n, l) in texto.split('\n').enumerate() {
        let largo = l.chars().count();
        if n == linea {
            return cursor + columna.min(largo);
        }
        cursor += largo + 1;
    }
    texto.chars().count()
}

/// Aplica una tecla al texto y al cursor (salvo `Terminar`, que no hace nada acá).
fn editar(texto: &mut String, cursor: &mut usize, t: &Tecla) {
    let total = texto.chars().count();
    *cursor = (*cursor).min(total);
    let (linea, columna) = linea_columna(texto, *cursor);
    let lineas = texto.split('\n').count();
    match t {
        Tecla::Escribir(s) => {
            texto.insert_str(byte(texto, *cursor), s);
            *cursor += s.chars().count();
        }
        Tecla::Borrar if *cursor > 0 => {
            texto.remove(byte(texto, *cursor - 1));
            *cursor -= 1;
        }
        Tecla::Suprimir if *cursor < total => {
            texto.remove(byte(texto, *cursor));
        }
        Tecla::Izquierda => *cursor = cursor.saturating_sub(1),
        Tecla::Derecha => *cursor = (*cursor + 1).min(total),
        Tecla::Arriba if linea > 0 => *cursor = en_linea(texto, linea - 1, columna),
        Tecla::Abajo if linea + 1 < lineas => *cursor = en_linea(texto, linea + 1, columna),
        Tecla::Inicio => *cursor = en_linea(texto, linea, 0),
        Tecla::Fin => *cursor = en_linea(texto, linea, usize::MAX),
        _ => {}
    }
}

impl Editor {
    /// Crea un texto vacío con la línea base empezando en `p` (mm) y empieza a escribirlo.
    pub(crate) fn new_text_at(&mut self, p: Point) {
        self.finish_text();
        let Some(fuente) = self
            .fuente
            .clone()
            .or_else(fuentes::predeterminada)
            .or_else(|| self.pick_font())
        else {
            return;
        };
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        let obj = Objeto {
            nombre: "texto".into(),
            visible: true,
            color: 1,
            forma: Forma::Texto {
                texto: String::new(),
                fuente,
                alto: 10.0,
                espaciado: 0.0,
                afin: Affine::translate(p.to_vec2()).as_coeffs(),
            },
            puntada: preset_por_nombre("Relleno horizontal"),
            opacidad: bordado::OPACIDAD_GUIA,
            bloqueado: false,
        };
        let antes = doc.history_len();
        doc.add(vec![obj]);
        self.escritura = Some(Escritura {
            obj: doc.proyecto.objetos.len() - 1,
            nuevo: Some(antes),
            cursor: 0,
        });
        self.info("Escribe el texto · Enter: otra línea · Esc o clic afuera: terminar");
    }

    /// Sigue escribiendo el texto `i`, con el cursor al final.
    pub(crate) fn edit_text(&mut self, i: usize) {
        self.finish_text();
        if let Some(doc) = &mut self.proyecto {
            doc.seleccion.fill(false);
            doc.seleccion[i] = true;
            let cursor = match &doc.proyecto.objetos[i].forma {
                Forma::Texto { texto, .. } => texto.chars().count(),
                _ => 0,
            };
            self.escritura = Some(Escritura {
                obj: i,
                nuevo: None,
                cursor,
            });
        }
    }

    /// El texto en edición y la parte antes del cursor (para dibujar el cursor).
    pub(crate) fn writing_cursor(&self) -> Option<(usize, String)> {
        let i = self.writing()?;
        let doc = self.proyecto.as_ref()?;
        let Forma::Texto { texto, .. } = &doc.proyecto.objetos[i].forma else {
            return None;
        };
        let cursor = self.escritura?.cursor;
        Some((i, texto.chars().take(cursor).collect()))
    }

    /// Termina de escribir. Un texto nuevo que quedó vacío se descarta (también del historial).
    pub(crate) fn finish_text(&mut self) {
        let (Some(e), Some(doc)) = (self.escritura.take(), self.proyecto.as_mut()) else {
            return;
        };
        let vacio = matches!(
            doc.proyecto.objetos.get(e.obj).map(|o| &o.forma),
            Some(Forma::Texto { texto, .. }) if texto.trim().is_empty()
        );
        if vacio {
            match e.nuevo {
                Some(len) => doc.discard_since(len),
                None => doc.remove(e.obj),
            }
        }
        doc.end_group();
    }

    /// El texto en edición, si sigue existiendo (se pudo borrar o deshacer).
    pub(crate) fn writing(&self) -> Option<usize> {
        let e = self.escritura?;
        let o = self.proyecto.as_ref()?.proyecto.objetos.get(e.obj)?;
        matches!(o.forma, Forma::Texto { .. }).then_some(e.obj)
    }

    /// Teclas mientras se escribe en el lienzo: van al texto en vez de a los atajos.
    pub(crate) fn text_keys(&mut self, ctx: &egui::Context) {
        let Some(i) = self.writing() else {
            self.escritura = None;
            return;
        };
        // Si un campo del panel tiene el foco, las teclas son de él.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mut teclas = Vec::new();
        ctx.input_mut(|inp| {
            inp.events.retain(|e| {
                let t = match e {
                    Event::Text(t) | Event::Paste(t) => Tecla::Escribir(t.clone()),
                    Event::Key {
                        key, pressed: true, ..
                    } => match key {
                        Key::Backspace => Tecla::Borrar,
                        Key::Delete => Tecla::Suprimir,
                        Key::Enter => Tecla::Escribir("\n".into()),
                        Key::ArrowLeft => Tecla::Izquierda,
                        Key::ArrowRight => Tecla::Derecha,
                        Key::ArrowUp => Tecla::Arriba,
                        Key::ArrowDown => Tecla::Abajo,
                        Key::Home => Tecla::Inicio,
                        Key::End => Tecla::Fin,
                        Key::Escape => Tecla::Terminar,
                        _ => return true,
                    },
                    _ => return true,
                };
                teclas.push(t);
                false
            });
        });
        if teclas.is_empty() {
            return;
        }
        let (Some(doc), Some(e)) = (&self.proyecto, self.escritura) else {
            return;
        };
        let Forma::Texto { texto, .. } = &doc.proyecto.objetos[i].forma else {
            return;
        };
        let mut texto = texto.clone();
        let mut cursor = e.cursor;
        let mut terminar = false;
        for t in &teclas {
            if *t == Tecla::Terminar {
                terminar = true;
                break;
            }
            editar(&mut texto, &mut cursor, t);
        }
        if let Some(e) = &mut self.escritura {
            e.cursor = cursor;
        }
        self.apply_text(i, texto);
        if terminar {
            self.finish_text();
            self.set_tool(crate::Tool::Select);
        }
    }

    fn apply_text(&mut self, i: usize, nuevo: String) {
        let Some(doc) = &mut self.proyecto else {
            return;
        };
        if let Forma::Texto { texto, .. } = &doc.proyecto.objetos[i].forma
            && *texto != nuevo
        {
            doc.edit_grouped(format!("escribir {i}"), |p| {
                if let Forma::Texto { texto, .. } = &mut p.objetos[i].forma {
                    *texto = nuevo;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn teclear(texto: &str, cursor: usize, teclas: &[Tecla]) -> (String, usize) {
        let (mut t, mut c) = (texto.to_string(), cursor);
        for k in teclas {
            editar(&mut t, &mut c, k);
        }
        (t, c)
    }

    #[test]
    fn writes_and_deletes_at_the_cursor() {
        let e = |s: &str| Tecla::Escribir(s.into());
        assert_eq!(
            teclear(
                "",
                0,
                &[e("Hla"), Tecla::Izquierda, Tecla::Izquierda, e("o")]
            ),
            ("Hola".into(), 2)
        );
        assert_eq!(
            teclear("año", 3, &[Tecla::Izquierda, Tecla::Borrar]),
            ("ao".into(), 1)
        );
        assert_eq!(teclear("año", 1, &[Tecla::Suprimir]), ("ao".into(), 1));
        // En los extremos no pasa nada.
        assert_eq!(
            teclear("ab", 0, &[Tecla::Borrar, Tecla::Izquierda]),
            ("ab".into(), 0)
        );
        assert_eq!(
            teclear("ab", 2, &[Tecla::Suprimir, Tecla::Derecha]),
            ("ab".into(), 2)
        );
    }

    #[test]
    fn moves_between_lines_keeping_the_column() {
        let t = "Hola\nyo\nmundo";
        // Desde "mun|do" hacia arriba: "yo|" (la línea es más corta), y de nuevo: "Ho|la".
        assert_eq!(teclear(t, 11, &[Tecla::Arriba]).1, 7);
        assert_eq!(teclear(t, 11, &[Tecla::Arriba, Tecla::Arriba]).1, 2);
        assert_eq!(teclear(t, 2, &[Tecla::Abajo]).1, 7);
        assert_eq!(
            teclear(t, 2, &[Tecla::Arriba]).1,
            2,
            "ya está en la primera"
        );
        assert_eq!(teclear(t, 6, &[Tecla::Fin]).1, 7);
        assert_eq!(teclear(t, 6, &[Tecla::Inicio]).1, 5);
        assert_eq!(
            teclear(t, 11, &[Tecla::Abajo]).1,
            11,
            "ya está en la última"
        );
    }
}
