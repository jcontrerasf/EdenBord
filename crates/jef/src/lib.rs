//! Lectura y escritura de archivos de bordado Janome JEF.
//!
//! ```no_run
//! let bytes = std::fs::read("diseño.jef").unwrap();
//! let design = jef::read(&bytes).unwrap();
//! assert_eq!(jef::write(&design), bytes);
//! ```

mod edit;
mod model;
pub mod palette;
mod read;
mod write;

pub use edit::{Block, SAFE_SCALE};
pub use model::{Bounds, Header, Hoop, Jef, Stitch, StitchKind};
pub use read::read;
pub use write::write;

/// Posición de la tabla de colores dentro de la cabecera.
const COLOR_TABLE_OFFSET: usize = 116;
/// Tipo de hilo que escriben todos los archivos conocidos.
const DEFAULT_THREAD_TYPE: i32 = 13;
/// Mayor desplazamiento por eje que cabe en un byte sin chocar con el escape `0x80`.
pub const MAX_DELTA: i32 = 127;

const CTRL_ESCAPE: u8 = 0x80;
const CTRL_COLOR: u8 = 0x01;
const CTRL_JUMP: u8 = 0x02;
const CTRL_END: u8 = 0x10;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JefError {
    #[error("archivo truncado: faltan datos en el byte {at}")]
    Truncated { at: usize },
    #[error("número de colores inválido: {0}")]
    BadColorCount(i32),
    #[error("offset de puntadas inválido: {0}")]
    BadOffset(i32),
    #[error("código de control desconocido 0x{code:02x} en el byte {at}")]
    UnknownControl { code: u8, at: usize },
    #[error("las puntadas terminan sin la marca de fin")]
    MissingEnd,
}

/// Lee un archivo JEF desde disco.
pub fn read_file(path: impl AsRef<std::path::Path>) -> Result<Jef, ReadFileError> {
    let bytes = std::fs::read(path)?;
    Ok(read(&bytes)?)
}

#[derive(Debug, thiserror::Error)]
pub enum ReadFileError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Jef(#[from] JefError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design(stitches: &[(i32, i32, StitchKind)], colors: Vec<i32>) -> Jef {
        let mut jef = Jef {
            header: Header {
                flags: 0x0A,
                datetime: [0; 16],
                hoop: Hoop::A110x110,
                extents: [[0; 4]; 5],
                thread_types: vec![],
                gap: vec![],
            },
            colors,
            stitches: stitches
                .iter()
                .map(|&(x, y, kind)| Stitch { x, y, kind })
                .collect(),
            trailing: vec![],
        };
        jef.refresh_header();
        jef
    }

    #[test]
    fn simple_roundtrip() {
        use StitchKind::*;
        let jef = design(
            &[
                (10, 0, Normal),
                (10, -20, Normal),
                (60, 40, Jump),
                (60, 40, ColorChange),
                (0, 0, Normal),
            ],
            vec![1, 10],
        );
        let bytes = write(&jef);
        assert_eq!(read(&bytes).unwrap(), jef);
        // 4 puntadas/saltos/cambios: normal=1 palabra, control=2 palabras, + fin.
        assert_eq!(
            i32::from_le_bytes(bytes[28..32].try_into().unwrap()),
            1 + 1 + 2 + 2 + 1 + 1
        );
    }

    #[test]
    fn long_moves_are_split() {
        use StitchKind::*;
        let jef = design(
            &[
                (500, -300, Jump),
                (500, -10, Normal),
                (900, 300, ColorChange),
            ],
            vec![1, 2],
        );
        let back = read(&write(&jef)).unwrap();

        // Salto de 500 → 4 tramos; cambio de color a 400 de distancia → 4 saltos + el cambio en (0,0).
        assert_eq!(
            back.stitches[3],
            Stitch {
                x: 500,
                y: -300,
                kind: Jump
            }
        );
        assert_eq!(back.count(Jump), 4 + 4);
        assert_eq!(back.count(ColorChange), 1);
        assert_eq!(
            back.stitches.last().unwrap(),
            &Stitch {
                x: 900,
                y: 300,
                kind: ColorChange
            }
        );
        // El tramo normal de 290 se parte en 3 puntadas normales.
        assert_eq!(back.count(Normal), 3);
        let mut prev = (0, 0);
        for s in &back.stitches {
            assert!((s.x - prev.0).abs() <= MAX_DELTA && (s.y - prev.1).abs() <= MAX_DELTA);
            prev = (s.x, s.y);
        }
    }

    #[test]
    fn refresh_header_extents_and_hoop() {
        use StitchKind::*;
        let mut jef = design(&[(-410, -246, Normal), (299, 148, Normal)], vec![1]);
        assert_eq!(jef.header.extents[0], [410, 246, 299, 148]);
        assert_eq!(jef.header.extents[1], [220, 304, 331, 402]);
        assert_eq!(jef.header.extents[2], [-1; 4]);
        assert_eq!(jef.header.extents[3], [290, 754, 401, 852]);
        assert_eq!(jef.header.hoop, Hoop::A110x110);

        jef.stitches.push(Stitch {
            x: 0,
            y: 900,
            kind: Normal,
        });
        jef.refresh_header();
        assert_eq!(jef.header.hoop, Hoop::B140x200);
        assert_eq!(jef.header.thread_types, vec![DEFAULT_THREAD_TYPE]);
    }

    #[test]
    fn corrupt_input_errors_instead_of_panicking() {
        use StitchKind::*;
        let good = write(&design(&[(10, 10, Normal), (20, 20, Jump)], vec![1]));
        for len in 0..good.len() {
            assert!(read(&good[..len]).is_err(), "truncado a {len} no dio error");
        }

        let mut bad = good.clone();
        bad[24..28].copy_from_slice(&(-5i32).to_le_bytes());
        assert_eq!(read(&bad), Err(JefError::BadColorCount(-5)));

        let mut bad = good.clone();
        bad[0..4].copy_from_slice(&100i32.to_le_bytes());
        assert_eq!(read(&bad), Err(JefError::BadOffset(100)));

        let mut bad = good.clone();
        let at = good.len() - 1;
        bad[at] = 0x33;
        assert_eq!(
            read(&bad),
            Err(JefError::UnknownControl {
                code: 0x33,
                at: at - 1
            })
        );
    }
}
