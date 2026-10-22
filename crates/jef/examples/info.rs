//! Muestra un resumen de uno o más archivos JEF.
//!
//! `cargo run -p jef --example info -- archivo.jef [otro.jef ...]`

use jef::{StitchKind, palette};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("uso: info archivo.jef [otro.jef ...]");
        std::process::exit(2);
    }
    for path in paths {
        let design = match jef::read_file(&path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{path}: {e}");
                continue;
            }
        };
        let (w, h) = design.size_mm();
        let date = &design.header.datetime[..14];
        let date = if date.iter().all(|&b| b == 0) {
            "(sin fecha)".to_string()
        } else {
            String::from_utf8_lossy(date).into_owned()
        };
        println!("{path}");
        println!("  tamaño:    {w:.1} × {h:.1} mm");
        println!("  bastidor:  {}", design.header.hoop.name());
        println!("  fecha:     {date}");
        println!(
            "  puntadas:  {}  saltos: {}  colores: {}",
            design.count(StitchKind::Normal),
            design.count(StitchKind::Jump),
            design.colors.len()
        );
        for (i, block) in design.color_blocks().iter().enumerate() {
            let idx = design.colors.get(i).copied().unwrap_or(0);
            let name = palette::thread(idx).map_or("?", |t| t.name);
            let [r, g, b] = design.thread_rgb(i);
            println!(
                "    {:>2}. #{r:02X}{g:02X}{b:02X} {name} (Janome {idx}) — {} puntadas",
                i + 1,
                block.len()
            );
        }
        if !design.trailing.is_empty() {
            println!(
                "  ¡ojo! {} bytes extra después del fin",
                design.trailing.len()
            );
        }
    }
}
