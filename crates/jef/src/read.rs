use crate::model::{Header, Hoop, Jef, Stitch, StitchKind};
use crate::{COLOR_TABLE_OFFSET, CTRL_COLOR, CTRL_END, CTRL_ESCAPE, CTRL_JUMP, JefError};

/// Más colores que esto indica un archivo corrupto, no un diseño real.
const MAX_COLORS: i32 = 1000;

fn i32_at(data: &[u8], pos: usize) -> Result<i32, JefError> {
    data.get(pos..pos + 4)
        .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(JefError::Truncated { at: pos })
}

/// Lee un archivo JEF completo desde memoria.
pub fn read(data: &[u8]) -> Result<Jef, JefError> {
    let offset = i32_at(data, 0)?;
    let flags = i32_at(data, 4)?;
    let datetime: [u8; 16] = data
        .get(8..24)
        .ok_or(JefError::Truncated { at: 8 })?
        .try_into()
        .unwrap();
    let color_count = i32_at(data, 24)?;
    // 28: número de puntadas, se recalcula al escribir.
    let hoop = Hoop::from_code(i32_at(data, 32)?);

    let mut extents = [[0; 4]; 5];
    for (b, block) in extents.iter_mut().enumerate() {
        for (k, v) in block.iter_mut().enumerate() {
            *v = i32_at(data, 36 + (b * 4 + k) * 4)?;
        }
    }

    if !(0..=MAX_COLORS).contains(&color_count) {
        return Err(JefError::BadColorCount(color_count));
    }
    let n = color_count as usize;
    let colors = (0..n)
        .map(|i| i32_at(data, COLOR_TABLE_OFFSET + i * 4))
        .collect::<Result<Vec<_>, _>>()?;
    let thread_types = (0..n)
        .map(|i| i32_at(data, COLOR_TABLE_OFFSET + (n + i) * 4))
        .collect::<Result<Vec<_>, _>>()?;

    let table_end = COLOR_TABLE_OFFSET + 8 * n;
    let offset = usize::try_from(offset)
        .ok()
        .filter(|&o| o >= table_end && o <= data.len())
        .ok_or(JefError::BadOffset(offset))?;
    let gap = data[table_end..offset].to_vec();

    let (stitches, end) = read_stitches(data, offset)?;

    Ok(Jef {
        header: Header {
            flags,
            datetime,
            hoop,
            extents,
            thread_types,
            gap,
        },
        colors,
        stitches,
        trailing: data[end..].to_vec(),
    })
}

/// Decodifica la secuencia de puntadas. Devuelve las puntadas y la posición después del fin.
fn read_stitches(data: &[u8], mut pos: usize) -> Result<(Vec<Stitch>, usize), JefError> {
    let mut stitches = Vec::new();
    let (mut x, mut y) = (0i32, 0i32);
    loop {
        let pair = data.get(pos..pos + 2).ok_or(JefError::MissingEnd)?;
        let kind = if pair[0] == CTRL_ESCAPE {
            match pair[1] {
                CTRL_END => return Ok((stitches, pos + 2)),
                CTRL_COLOR => StitchKind::ColorChange,
                CTRL_JUMP => StitchKind::Jump,
                code => return Err(JefError::UnknownControl { code, at: pos }),
            }
        } else {
            StitchKind::Normal
        };
        if kind != StitchKind::Normal {
            pos += 2;
        }
        let d = data.get(pos..pos + 2).ok_or(JefError::MissingEnd)?;
        x += i32::from(d[0] as i8);
        // En el archivo la y crece hacia arriba; en el modelo, hacia abajo.
        y -= i32::from(d[1] as i8);
        pos += 2;
        stitches.push(Stitch { x, y, kind });
    }
}
