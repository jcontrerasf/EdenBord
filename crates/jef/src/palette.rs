//! Paleta fija de hilos Janome usada por los archivos JEF.
//!
//! Los datos vienen de pyembroidery (`EmbThreadJef.py`), licencia MIT:
//! <https://github.com/EmbroidePy/pyembroidery> — Copyright (c) 2018 pyembroidery contributors.

/// Un hilo de la paleta Janome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thread {
    pub rgb: [u8; 3],
    pub name: &'static str,
    pub catalog: &'static str,
}

const fn t(rgb: u32, name: &'static str, catalog: &'static str) -> Thread {
    Thread {
        rgb: [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8],
        name,
        catalog,
    }
}

/// Paleta indexada por el número de color guardado en el JEF (78 hilos).
/// El índice 0 no es un hilo real.
pub const PALETTE: &[Thread] = &[
    t(0x000000, "Placeholder", "000"),
    t(0x000000, "Black", "002"),
    t(0xFFFFFF, "White", "001"),
    t(0xFFFF17, "Yellow", "204"),
    t(0xFF6600, "Orange", "203"),
    t(0x2F5933, "Olive Green", "219"),
    t(0x237336, "Green", "226"),
    t(0x65C2C8, "Sky", "217"),
    t(0xAB5A96, "Purple", "208"),
    t(0xF669A0, "Pink", "201"),
    t(0xFF0000, "Red", "225"),
    t(0xB1704E, "Brown", "214"),
    t(0x0B2F84, "Blue", "207"),
    t(0xE4C35D, "Gold", "003"),
    t(0x481A05, "Dark Brown", "205"),
    t(0xAC9CC7, "Pale Violet", "209"),
    t(0xFCF294, "Pale Yellow", "210"),
    t(0xF999B7, "Pale Pink", "211"),
    t(0xFAB381, "Peach", "212"),
    t(0xC9A480, "Beige", "213"),
    t(0x970533, "Wine Red", "215"),
    t(0xA0B8CC, "Pale Sky", "216"),
    t(0x7FC21C, "Yellow Green", "218"),
    t(0xE5E5E5, "Silver Gray", "220"),
    t(0x889B9B, "Gray", "221"),
    t(0x98D6BD, "Pale Aqua", "227"),
    t(0xB2E1E3, "Baby Blue", "228"),
    t(0x368BA0, "Powder Blue", "229"),
    t(0x4F83AB, "Bright Blue", "230"),
    t(0x386A91, "Slate Blue", "231"),
    t(0x071650, "Navy Blue", "232"),
    t(0xF999A2, "Salmon Pink", "233"),
    t(0xF9676B, "Coral", "234"),
    t(0xE3311F, "Burnt Orange", "235"),
    t(0xE2A188, "Cinnamon", "236"),
    t(0xB59474, "Umber", "237"),
    t(0xE4CF99, "Blond", "238"),
    t(0xFFCB00, "Sunflower", "239"),
    t(0xE1ADD4, "Orchid Pink", "240"),
    t(0xC3007E, "Peony Purple", "241"),
    t(0x80004B, "Burgundy", "242"),
    t(0x540571, "Royal Purple", "243"),
    t(0xB10525, "Cardinal Red", "244"),
    t(0xCAE0C0, "Opal Green", "245"),
    t(0x899856, "Moss Green", "246"),
    t(0x5C941A, "Meadow Green", "247"),
    t(0x003114, "Dark Green", "248"),
    t(0x5DAE94, "Aquamarine", "249"),
    t(0x4CBF8F, "Emerald Green", "250"),
    t(0x007772, "Peacock Green", "251"),
    t(0x595B61, "Dark Gray", "252"),
    t(0xFFFFF2, "Ivory White", "253"),
    t(0xB15818, "Hazel", "254"),
    t(0xCB8A07, "Toast", "255"),
    t(0x986C80, "Salmon", "256"),
    t(0x98692D, "Cocoa Brown", "257"),
    t(0x4D3419, "Sienna", "258"),
    t(0x4C330B, "Sepia", "259"),
    t(0x33200A, "Dark Sepia", "260"),
    t(0x523A97, "Violet Blue", "261"),
    t(0x0D217E, "Blue Ink", "262"),
    t(0x1E77AC, "Sola Blue", "263"),
    t(0xB2DD53, "Green Dust", "264"),
    t(0xF33689, "Crimson", "265"),
    t(0xDE649E, "Floral Pink", "266"),
    t(0x984161, "Wine", "267"),
    t(0x4C5612, "Olive Drab", "268"),
    t(0x4C881F, "Meadow", "269"),
    t(0xE4DE79, "Mustard", "270"),
    t(0xCB8A1A, "Yellow Ocher", "271"),
    t(0xCBA21C, "Old Gold", "272"),
    t(0xFF9805, "Honey Dew", "273"),
    t(0xFCB257, "Tangerine", "274"),
    t(0xFFE505, "Canary Yellow", "275"),
    t(0xF0331F, "Vermilion", "202"),
    t(0x1A842D, "Bright Green", "206"),
    t(0x386CAE, "Ocean Blue", "222"),
    t(0xE3C4B4, "Beige Gray", "223"),
    t(0xE3AC81, "Bamboo", "224"),
];

/// Devuelve el hilo para un índice JEF, o `None` si está fuera de la paleta.
pub fn thread(index: i32) -> Option<&'static Thread> {
    usize::try_from(index).ok().and_then(|i| PALETTE.get(i))
}

/// Índice de la paleta (1..) cuyo color se parece más a `rgb`.
pub fn nearest(rgb: [u8; 3]) -> i32 {
    let dist = |c: [u8; 3]| -> i32 {
        (0..3)
            .map(|k| (i32::from(c[k]) - i32::from(rgb[k])).pow(2))
            .sum()
    };
    (1..PALETTE.len())
        .min_by_key(|&i| dist(PALETTE[i].rgb))
        .map_or(1, |i| i as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_entries() {
        assert_eq!(PALETTE.len(), 79);
        assert_eq!(thread(1).unwrap().name, "Black");
        assert_eq!(thread(10).unwrap().rgb, [0xFF, 0, 0]);
        assert!(thread(-1).is_none());
        assert!(thread(79).is_none());
        assert_eq!(nearest([250, 5, 5]), 10);
    }
}
