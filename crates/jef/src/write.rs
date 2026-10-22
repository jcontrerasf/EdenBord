use crate::model::{Jef, StitchKind};
use crate::{
    COLOR_TABLE_OFFSET, CTRL_COLOR, CTRL_END, CTRL_ESCAPE, CTRL_JUMP, DEFAULT_THREAD_TYPE,
    MAX_DELTA,
};

/// Escribe el diseño en formato JEF.
///
/// Los conteos y el offset se calculan; el resto de la cabecera sale de `jef.header`.
/// Si modificaste las puntadas, llama antes a [`Jef::refresh_header`].
pub fn write(jef: &Jef) -> Vec<u8> {
    let body = encode_stitches(jef);
    let n = jef.colors.len();
    let offset = COLOR_TABLE_OFFSET + 8 * n + jef.header.gap.len();

    let mut out = Vec::with_capacity(offset + body.len() + jef.trailing.len());
    let put = |v: i32, out: &mut Vec<u8>| out.extend_from_slice(&v.to_le_bytes());
    put(offset as i32, &mut out);
    put(jef.header.flags, &mut out);
    out.extend_from_slice(&jef.header.datetime);
    put(n as i32, &mut out);
    put((body.len() / 2) as i32, &mut out);
    put(jef.header.hoop.code(), &mut out);
    for v in jef.header.extents.as_flattened() {
        put(*v, &mut out);
    }
    for &c in &jef.colors {
        put(c, &mut out);
    }
    for i in 0..n {
        let t = jef.header.thread_types.get(i).copied();
        put(t.unwrap_or(DEFAULT_THREAD_TYPE), &mut out);
    }
    out.extend_from_slice(&jef.header.gap);
    out.extend_from_slice(&body);
    out.extend_from_slice(&jef.trailing);
    out
}

/// Codifica las puntadas como desplazamientos relativos, terminando con el fin.
/// Los movimientos más largos que [`MAX_DELTA`] se parten en tramos iguales.
fn encode_stitches(jef: &Jef) -> Vec<u8> {
    let mut out = Vec::with_capacity(jef.stitches.len() * 2 + 2);
    let (mut px, mut py) = (0i32, 0i32);
    for s in &jef.stitches {
        let (dx, dy) = (s.x - px, s.y - py);
        let steps = (dx.abs().max(dy.abs()) + MAX_DELTA - 1) / MAX_DELTA;
        let mut prev = (0, 0);
        for k in 1..=steps.max(1) {
            let cur = (dx * k / steps.max(1), dy * k / steps.max(1));
            let last = k == steps.max(1);
            // Un cambio de color lejano se hace saltando primero y cambiando al llegar.
            let kind = match s.kind {
                StitchKind::ColorChange if !last => StitchKind::Jump,
                kind => kind,
            };
            match kind {
                StitchKind::Normal => {}
                StitchKind::Jump => out.extend_from_slice(&[CTRL_ESCAPE, CTRL_JUMP]),
                StitchKind::ColorChange => out.extend_from_slice(&[CTRL_ESCAPE, CTRL_COLOR]),
            }
            // y hacia abajo en el modelo, hacia arriba en el archivo.
            out.push((cur.0 - prev.0) as i8 as u8);
            out.push((-(cur.1 - prev.1)) as i8 as u8);
            prev = cur;
        }
        (px, py) = (s.x, s.y);
    }
    out.extend_from_slice(&[CTRL_ESCAPE, CTRL_END]);
    out
}
