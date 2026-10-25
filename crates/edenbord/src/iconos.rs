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
pub const DESBLOQUEADO: &str = "\u{E306}"; // lock-open
pub const VISIBLE: &str = "\u{E220}"; // eye
pub const OCULTO: &str = "\u{E224}"; // eye-slash
pub const MENU: &str = "\u{E2F0}"; // list
pub const RECIENTES: &str = "\u{E1A0}"; // clock-counter-clockwise
pub const ARCHIVO: &str = "\u{E230}"; // file
pub const ARCHIVO_JEF: &str = "\u{E82E}"; // needle
pub const FLOR: &str = "\u{EACC}"; // flower-tulip
pub const ACERCAR: &str = "\u{E310}"; // magnifying-glass-plus
pub const ALEJAR: &str = "\u{E30E}"; // magnifying-glass-minus

// Herramientas.
pub const SELECCIONAR: &str = "\u{E1DC}"; // cursor
pub const NODOS: &str = "\u{EB00}"; // bezier-curve
pub const PLUMA: &str = "\u{E3AC}"; // pen-nib
pub const REGLA: &str = "\u{E6B8}"; // ruler
pub const CUADRICULA: &str = "\u{E296}"; // grid-four
pub const AJUSTAR: &str = "\u{E1D0}"; // corners-out

// Vista.
pub const SALTOS: &str = "\u{E016}"; // arrow-arc-right
pub const BASTIDOR: &str = "\u{E626}"; // frame-corners
pub const REALISTA: &str = "\u{E6A2}"; // sparkle
pub const CONTORNOS: &str = "\u{E69A}"; // selection

// Transformar.
pub const GIRAR_IZQ: &str = "\u{E038}"; // arrow-counter-clockwise
pub const GIRAR_DER: &str = "\u{E036}"; // arrow-clockwise
pub const ESPEJO_H: &str = "\u{ED6A}"; // flip-horizontal
pub const ESPEJO_V: &str = "\u{ED6C}"; // flip-vertical
pub const CENTRAR: &str = "\u{E1D6}"; // crosshair
pub const SUBIR: &str = "\u{E13C}"; // caret-up
pub const BAJAR: &str = "\u{E136}"; // caret-down

pub const PROPORCIONAL: &str = "\u{E2EA}"; // link-simple-horizontal
pub const LIBRE: &str = "\u{E2EC}"; // link-simple-horizontal-break
pub const PASO: &str = "\u{E0A4}"; // arrows-out-cardinal
pub const ESCALAR: &str = "\u{ED6E}"; // resize
pub const GIRAR: &str = "\u{E7BC}"; // angle

// Trazados.
pub const COMBINAR: &str = "\u{E468}"; // stack-simple
pub const SEPARAR: &str = "\u{EAE0}"; // scissors
pub const ESQUINA_CURVA: &str = "\u{E39C}"; // path
pub const UNIR: &str = "\u{E87E}"; // unite
pub const RESTAR: &str = "\u{EBD6}"; // subtract
pub const INTERSECAR: &str = "\u{E2D2}"; // intersect
pub const EXCLUIR: &str = "\u{E882}"; // exclude
pub const CERRAR: &str = "\u{E6D0}"; // polygon
pub const LISTO: &str = "\u{E182}"; // check
pub const CANCELAR: &str = "\u{E4F6}"; // x

// Tipos de puntada.
pub const GUIA: &str = "\u{E602}"; // circle-dashed
pub const CORRIDO: &str = "\u{E6D4}"; // line-segments
pub const SATIN: &str = "\u{EA9C}"; // wave-sawtooth
pub const RELLENO: &str = "\u{E392}"; // paint-bucket
pub const MANUAL: &str = "\u{E29A}"; // hand-pointing
pub const PUNTO_ATRAS: &str = "\u{E2E6}"; // link-simple
pub const TALLO: &str = "\u{EBAE}"; // plant
pub const FESTON: &str = "\u{E9C0}"; // feather
pub const MOTIVO: &str = "\u{EA9A}"; // wave-sine
pub const DEGRADE: &str = "\u{EB42}"; // gradient
pub const CONCENTRICO: &str = "\u{E9FA}"; // spiral
pub const CUADRILLE: &str = "\u{E2A2}"; // hash

// Simulación.
pub const REPRODUCIR: &str = "\u{E3D0}"; // play
pub const PAUSA: &str = "\u{E39E}"; // pause
pub const AL_INICIO: &str = "\u{E5A4}"; // skip-back
pub const AL_FIN: &str = "\u{E5A6}"; // skip-forward
