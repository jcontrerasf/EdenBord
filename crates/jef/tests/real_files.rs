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
