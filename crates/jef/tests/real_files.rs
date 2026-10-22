//! Pruebas con diseños reales de la carpeta indicada en `JEF_FIXTURES`
//! (ver `.cargo/config.example.toml`). Sin la variable, se saltan.

use std::path::PathBuf;

use jef::StitchKind;

fn real_files() -> Vec<PathBuf> {
    let Some(dir) = std::env::var_os("JEF_FIXTURES").map(PathBuf::from) else {
        eprintln!("JEF_FIXTURES no está definida, se saltan las pruebas con archivos reales");
        return vec![];
    };
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("JEF_FIXTURES={}: {e}", dir.display()));
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jef")))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no hay archivos .jef en {}",
        dir.display()
    );
    files
}

#[test]
fn roundtrip_is_byte_exact() {
    for path in real_files() {
        let bytes = std::fs::read(&path).unwrap();
        let design = jef::read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            jef::write(&design) == bytes,
            "{}: la escritura difiere",
            path.display()
        );
    }
}

#[test]
fn header_matches_contents() {
    for path in real_files() {
        let bytes = std::fs::read(&path).unwrap();
        let design = jef::read(&bytes).unwrap();
        let name = path.display();
        let header_i32 = |pos: usize| i32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());

        assert_eq!(
            header_i32(24) as usize,
            design.colors.len(),
            "{name}: colores"
        );
        assert_eq!(
            design.count(StitchKind::ColorChange) + 1,
            design.colors.len(),
            "{name}: cambios"
        );

        let words = design.count(StitchKind::Normal)
            + 2 * (design.count(StitchKind::Jump) + design.count(StitchKind::ColorChange))
            + 1;
        assert_eq!(header_i32(28) as usize, words, "{name}: puntadas");

        let b = design.bounds();
        assert_eq!(
            design.header.extents[0],
            [-b.min_x, -b.min_y, b.max_x, b.max_y],
            "{name}: extensiones"
        );
    }
}

/// Una puntada cosida: color, desde, hasta.
type Segment = (i32, (i32, i32), (i32, i32));

/// Segmentos cosidos (color, desde, hasta), ordenados.
fn sewn(jef: &jef::Jef) -> Vec<Segment> {
    let mut out = Vec::new();
    let (mut block, mut prev) = (0, (0, 0));
    for s in &jef.stitches {
        match s.kind {
            StitchKind::ColorChange => block += 1,
            StitchKind::Normal => out.push((jef.colors[block], prev, (s.x, s.y))),
            StitchKind::Jump => {}
        }
        prev = (s.x, s.y);
    }
    out.sort();
    out
}

#[test]
fn blocks_roundtrip_is_byte_exact() {
    for path in real_files() {
        let bytes = std::fs::read(&path).unwrap();
        let mut design = jef::read(&bytes).unwrap();
        design.set_blocks(design.blocks());
        assert!(
            jef::write(&design) == bytes,
            "{}: bloques no son identidad",
            path.display()
        );
    }
}

#[test]
fn edits_preserve_what_is_sewn() {
    for path in real_files() {
        let original = jef::read_file(&path).unwrap();
        let name = path.display();
        let n = original.colors.len();

        let mut d = original.clone();
        d.move_block(0, n - 1);
        d.move_block(n - 1, 0);
        assert_eq!(sewn(&d), sewn(&original), "{name}: reordenar ida y vuelta");

        let mut d = original.clone();
        d.mirror_x(&[]);
        d.mirror_x(&[]);
        assert_eq!(sewn(&d), sewn(&original), "{name}: espejo doble");

        // Girar 4 veces 90° puede correr el dibujo a lo más 0,1 mm por giro (centro entero).
        let mut d = original.clone();
        for _ in 0..4 {
            d.rotate(&[], 90.0);
        }
        let (a, b) = (sewn(&d), sewn(&original));
        assert_eq!(a.len(), b.len(), "{name}: giros");
        let (dx, dy) = (a[0].1.0 - b[0].1.0, a[0].1.1 - b[0].1.1);
        assert!(
            dx.abs() <= 4 && dy.abs() <= 4,
            "{name}: giros corren ({dx},{dy})"
        );
        let shifted: Vec<_> = b
            .iter()
            .map(|&(c, p, q)| (c, (p.0 + dx, p.1 + dy), (q.0 + dx, q.1 + dy)))
            .collect();
        assert_eq!(a, shifted, "{name}: giros cambian la forma");

        // Lo editado se puede escribir y releer igual.
        let bytes = jef::write(&d);
        assert_eq!(
            jef::write(&jef::read(&bytes).unwrap()),
            bytes,
            "{name}: reescritura"
        );
    }
}

#[test]
fn combining_designs_keeps_both() {
    let files = real_files();
    for pair in files.windows(2) {
        let (a, b) = (
            jef::read_file(&pair[0]).unwrap(),
            jef::read_file(&pair[1]).unwrap(),
        );
        let mut combined = a.clone();
        let added: Vec<usize> = combined.append(&b).collect();
        combined.translate(&added, 300, 0);
        assert_eq!(combined.colors.len(), a.colors.len() + b.colors.len());
        assert_eq!(sewn(&combined).len(), sewn(&a).len() + sewn(&b).len());

        let bytes = jef::write(&combined);
        let back = jef::read(&bytes).unwrap();
        assert_eq!(jef::write(&back), bytes, "{}", pair[1].display());
        assert_eq!(back.colors, combined.colors);
    }
}
