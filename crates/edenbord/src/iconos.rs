//! Íconos de Phosphor (<https://phosphoricons.com>) usados en la interfaz.
//!
//! La fuente va incrustada (ver `tema::instalar`) como respaldo de Inter, así que un ícono se
//! puede mezclar con texto: `format!("{ABRIR} Abrir…")`. Las variantes Regular y Fill usan
//! los mismos códigos; la rellena se pide con la familia [`crate::tema::FILL`].
//! Códigos tomados de `egui-phosphor` (que aún no tiene versión para egui 0.37).

// Archivo y edición.
pub const ABRIR: &str = "\u{E256}"; // folder-open
pub const NUEVO: &str = "\u{E236}"; // file-plus
pub const GUARDAR: &str = "\u{E248}"; // floppy-disk
pub const EXPORTAR: &str = "\u{EAF0}"; // export
pub const AGREGAR: &str = "\u{E3D4}"; // plus
pub const SVG: &str = "\u{ED08}"; // file-svg
pub const TEXTO: &str = "\u{E48A}"; // text-t
pub const DESHACER: &str = "\u{E08A}"; // arrow-u-up-left
pub const REHACER: &str = "\u{E08C}"; // arrow-u-up-right
pub const BORRAR: &str = "\u{E4A6}"; // trash
pub const DUPLICAR: &str = "\u{E1CA}"; // copy
pub const INFO: &str = "\u{E2CE}"; // info
pub const AVISO: &str = "\u{E4E0}"; // warning
pub const IMAGEN: &str = "\u{E2CA}"; // image
pub const BLOQUEADO: &str = "\u{E2FA}"; // lock

// Herramientas.
pub const SELECCIONAR: &str = "\u{E1DC}"; // cursor
pub const NODOS: &str = "\u{EB00}"; // bezier-curve
pub const PLUMA: &str = "\u{E3AC}"; // pen-nib
pub const REGLA: &str = "\u{E6B8}"; // ruler
pub const CUADRICULA: &str = "\u{E296}"; // grid-four

// Transformar.
pub const IZQUIERDA: &str = "\u{E058}"; // arrow-left
pub const ARRIBA: &str = "\u{E08E}"; // arrow-up
pub const ABAJO: &str = "\u{E03E}"; // arrow-down
pub const DERECHA: &str = "\u{E06C}"; // arrow-right
pub const GIRAR_IZQ: &str = "\u{E038}"; // arrow-counter-clockwise
pub const GIRAR_DER: &str = "\u{E036}"; // arrow-clockwise
pub const ESPEJO_H: &str = "\u{ED6A}"; // flip-horizontal
pub const ESPEJO_V: &str = "\u{ED6C}"; // flip-vertical
pub const CENTRAR: &str = "\u{E1D6}"; // crosshair
pub const SUBIR: &str = "\u{E13C}"; // caret-up
pub const BAJAR: &str = "\u{E136}"; // caret-down

// Trazados.
pub const COMBINAR: &str = "\u{E87E}"; // unite
pub const SEPARAR: &str = "\u{EAE0}"; // scissors
pub const ESQUINA_CURVA: &str = "\u{E39C}"; // path

// Tipos de objeto.
pub const ZONA: &str = "\u{E6D0}"; // polygon
pub const LINEA: &str = "\u{E6D4}"; // line-segments

// Simulación.
pub const REPRODUCIR: &str = "\u{E3D0}"; // play
pub const PAUSA: &str = "\u{E39E}"; // pause
pub const AL_INICIO: &str = "\u{E5A4}"; // skip-back
pub const AL_FIN: &str = "\u{E5A6}"; // skip-forward
