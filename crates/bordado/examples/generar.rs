//! Genera un JEF desde un proyecto `.bordado` o un SVG, sin abrir el editor.
//!
//! `cargo run -p bordado --example generar -- entrada.(bordado|svg) salida.jef [vista.svg]`
//!
//! A un SVG se le asigna relleno a las zonas y pespunte triple a las líneas. Con `vista.svg`
//! se dibujan además las puntadas, para revisarlas en cualquier visor.

use std::fmt::Write;

use bordado::{PRESETS, Proyecto, Puntada};
use jef::StitchKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [entrada, salida, rest @ ..] = args.as_slice() else {
        eprintln!("uso: generar entrada.(bordado|svg) salida.jef [vista.svg]");
        std::process::exit(2);
    };
    let proyecto = if entrada.to_lowercase().ends_with(".svg") {
        let preset = |n: &str| PRESETS.iter().find(|(p, _)| *p == n).unwrap().1.clone();
        let mut objetos = bordado::svg::importar_archivo(entrada)?;
        for o in &mut objetos {
            o.puntada = if o.es_zona() {
                preset("Relleno 45°")
            } else {
                preset("Pespunte triple")
            };
        }
        Proyecto {
            objetos,
            ..Default::default()
        }
    } else {
        Proyecto::leer_archivo(entrada)?
    };

    let g = proyecto.generar();
    for aviso in &g.avisos {
        eprintln!("aviso: {aviso}");
    }
    std::fs::write(salida, jef::write(&g.jef))?;
    let (w, h) = g.jef.size_mm();
    println!(
        "{salida}: {w:.1} × {h:.1} mm, {} puntadas, {} saltos, {} colores, bastidor {}",
        g.jef.count(StitchKind::Normal),
        g.jef.count(StitchKind::Jump),
        g.jef.colors.len(),
        g.jef.header.hoop.name()
    );
    let guias = proyecto
        .objetos
        .iter()
        .filter(|o| o.puntada == Puntada::Ninguna)
        .count();
    if guias > 0 {
        println!("{guias} objetos son guías y no se bordan");
    }

    if let Some(vista) = rest.first() {
        std::fs::write(vista, vista_svg(&g.jef))?;
    }
    Ok(())
}

/// Dibuja las puntadas (en 0,1 mm) como polilíneas, una por tramo cosido.
fn vista_svg(jef: &jef::Jef) -> String {
    let b = jef.bounds();
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}" height="{}">"#,
        b.min_x - 20,
        b.min_y - 20,
        b.width() + 40,
        b.height() + 40,
        b.width() + 40,
        b.height() + 40
    );
    let _ = write!(
        out,
        r##"<rect x="{}" y="{}" width="100%" height="100%" fill="#c4beb2"/>"##,
        b.min_x - 20,
        b.min_y - 20
    );
    let blocks = jef.color_blocks();
    let mut prev = (0, 0);
    for (k, range) in blocks.iter().enumerate() {
        let [r, g, bl] = jef.thread_rgb(k);
        let mut run: Vec<(i32, i32)> = vec![];
        let flush = |run: &mut Vec<(i32, i32)>, out: &mut String| {
            if run.len() > 1 {
                let pts: Vec<String> = run.iter().map(|(x, y)| format!("{x},{y}")).collect();
                let _ = write!(
                    out,
                    r#"<polyline fill="none" stroke="rgb({r},{g},{bl})" stroke-width="3" stroke-linejoin="round" points="{}"/>"#,
                    pts.join(" ")
                );
            }
            run.clear();
        };
        for s in &jef.stitches[range.clone()] {
            if s.kind == StitchKind::Normal {
                if run.is_empty() {
                    run.push(prev);
                }
                run.push((s.x, s.y));
            } else {
                flush(&mut run, &mut out);
            }
            prev = (s.x, s.y);
        }
        flush(&mut run, &mut out);
    }
    out.push_str("</svg>");
    out
}
