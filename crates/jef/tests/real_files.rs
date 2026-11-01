//! Pruebas con diseños reales de la carpeta indicada en `JEF_FIXTURES`
//! (ver `.cargo/config.example.toml`). Sin la variable, se saltan.

use std::path::{Path, PathBuf};

use jef::StitchKind;

fn real_files() -> Vec<PathBuf> {
    let Some(dir) = std::env::var_os("JEF_FIXTURES").map(PathBuf::from) else {
        eprintln!("JEF_FIXTURES no está definida, se saltan las pruebas con archivos reales");
        return vec![];
    };
    if !dir.is_dir() {
        panic!("JEF_FIXTURES={} no es una carpeta", dir.display());
    }
    let mut files = Vec::new();
    collect(&dir, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no hay archivos .jef en {}",
        dir.display()
    );
    files
}

/// `.jef` and `.jef+` files under `dir`, including subfolders.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        let ext = path.extension().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            collect(&path, out);
        } else if ext.eq_ignore_ascii_case("jef") || ext.eq_ignore_ascii_case("jef+") {
            out.push(path);
        }
    }
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
            + 2 * (design.count(StitchKind::Jump)
                + design.count(StitchKind::ColorChange)
                + design.count(StitchKind::Trim))
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
            StitchKind::Jump | StitchKind::Trim => {}
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

#[test]
fn stripped_jef_plus_is_a_plain_jef() {
    for path in real_files() {
        let original = jef::read_file(&path).unwrap();
        if !original.is_jef_plus() {
            continue;
        }
        let name = path.display();
        let mut d = original.clone();
        d.strip_jef_plus();
        assert!(!d.is_jef_plus() && d.trailing.is_empty(), "{name}");
        let bytes = jef::write(&d);
        let offset = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        assert_eq!(offset as usize, 116 + 8 * d.colors.len(), "{name}: offset");
        let back = jef::read(&bytes).unwrap();
        assert_eq!(back.stitches, original.stitches, "{name}: puntadas");
        assert_eq!(jef::write(&back), bytes, "{name}: reescritura");
    }
}
