use crate::palette;

/// Tipo de cada entrada de la secuencia de bordado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StitchKind {
    /// Puntada normal: la aguja baja en esta posición.
    Normal,
    /// Salto: la máquina se mueve sin coser.
    Jump,
    /// Cambio al siguiente color; los puntos que siguen usan el hilo siguiente.
    ColorChange,
}

/// Una entrada de la secuencia, en coordenadas absolutas (0,1 mm, y hacia abajo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stitch {
    pub x: i32,
    pub y: i32,
    pub kind: StitchKind,
}

/// Bastidores conocidos de Janome. Los nombres siguen las letras de Janome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hoop {
    /// Bastidor A, 110 × 110 mm.
    A110x110,
    /// Bastidor C, 50 × 50 mm.
    C50x50,
    /// Bastidor B, 140 × 200 mm.
    B140x200,
    /// Bastidor F, 126 × 110 mm.
    F126x110,
    /// Bastidor SQ, 200 × 200 mm.
    Sq200x200,
    Unknown(i32),
}

impl Hoop {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Hoop::A110x110,
            1 => Hoop::C50x50,
            2 => Hoop::B140x200,
            3 => Hoop::F126x110,
            4 => Hoop::Sq200x200,
            other => Hoop::Unknown(other),
        }
    }

    pub fn code(self) -> i32 {
        match self {
            Hoop::A110x110 => 0,
            Hoop::C50x50 => 1,
            Hoop::B140x200 => 2,
            Hoop::F126x110 => 3,
            Hoop::Sq200x200 => 4,
            Hoop::Unknown(code) => code,
        }
    }

    /// Ancho y alto en 0,1 mm, si el bastidor es conocido.
    pub fn size(self) -> Option<(i32, i32)> {
        match self {
            Hoop::A110x110 => Some((1100, 1100)),
            Hoop::C50x50 => Some((500, 500)),
            Hoop::B140x200 => Some((1400, 2000)),
            Hoop::F126x110 => Some((1260, 1100)),
            Hoop::Sq200x200 => Some((2000, 2000)),
            Hoop::Unknown(_) => None,
        }
    }

    pub fn name(self) -> String {
        match self {
            Hoop::A110x110 => "A 110×110 mm".into(),
            Hoop::C50x50 => "C 50×50 mm".into(),
            Hoop::B140x200 => "B 140×200 mm".into(),
            Hoop::F126x110 => "F 126×110 mm".into(),
            Hoop::Sq200x200 => "SQ 200×200 mm".into(),
            Hoop::Unknown(code) => format!("desconocido ({code})"),
        }
    }

    /// Bastidores en orden de tamaño, para elegir el más chico que calce.
    pub const BY_SIZE: [Hoop; 5] = [
        Hoop::C50x50,
        Hoop::A110x110,
        Hoop::F126x110,
        Hoop::B140x200,
        Hoop::Sq200x200,
    ];
}

/// Campos de la cabecera que no se pueden derivar de las puntadas.
/// Se conservan tal cual para que leer y escribir un archivo dé los mismos bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub flags: i32,
    /// Fecha y hora en ASCII `YYYYMMDDHHMMSS` más 2 bytes de relleno; a veces todo ceros.
    pub datetime: [u8; 16],
    pub hoop: Hoop,
    /// Bloque 0: distancias del centro al borde izquierdo, superior, derecho e inferior.
    /// Bloques 1–4: márgenes respecto a cada bastidor (`-1` = no cabe).
    pub extents: [[i32; 4]; 5],
    /// Un tipo de hilo por color (en la práctica siempre 13).
    pub thread_types: Vec<i32>,
    /// Bytes entre la tabla de colores y el inicio de las puntadas, si los hay.
    pub gap: Vec<u8>,
}

/// Rectángulo que contiene todas las posiciones, en 0,1 mm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Bounds {
    pub fn width(&self) -> i32 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> i32 {
        self.max_y - self.min_y
    }
}

/// Un diseño JEF completo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jef {
    pub header: Header,
    /// Índices a [`palette::PALETTE`], uno por bloque de color.
    pub colors: Vec<i32>,
    pub stitches: Vec<Stitch>,
    /// Bytes que siguen al fin del diseño. Algunos programas dejan basura ahí.
    pub trailing: Vec<u8>,
}

impl Jef {
    /// Rectángulo de todas las posiciones (incluye el origen, donde arranca la aguja).
    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds {
            min_x: 0,
            min_y: 0,
            max_x: 0,
            max_y: 0,
        };
        for s in &self.stitches {
            b.min_x = b.min_x.min(s.x);
            b.min_y = b.min_y.min(s.y);
            b.max_x = b.max_x.max(s.x);
            b.max_y = b.max_y.max(s.y);
        }
        b
    }

    /// Ancho y alto en milímetros.
    pub fn size_mm(&self) -> (f32, f32) {
        let b = self.bounds();
        (b.width() as f32 / 10.0, b.height() as f32 / 10.0)
    }

    /// Rangos de `stitches` que corresponden a cada color, en orden.
    /// Cada cambio de color abre un rango nuevo; el cambio en sí queda al inicio del rango nuevo.
    pub fn color_blocks(&self) -> Vec<std::ops::Range<usize>> {
        let mut blocks = Vec::new();
        let mut start = 0;
        for (i, s) in self.stitches.iter().enumerate() {
            if s.kind == StitchKind::ColorChange {
                blocks.push(start..i);
                start = i;
            }
        }
        blocks.push(start..self.stitches.len());
        blocks
    }

    /// Color RGB del bloque `block`, o gris si el índice no está en la paleta.
    pub fn thread_rgb(&self, block: usize) -> [u8; 3] {
        self.colors
            .get(block)
            .and_then(|&c| palette::thread(c))
            .map_or([128, 128, 128], |t| t.rgb)
    }

    pub fn count(&self, kind: StitchKind) -> usize {
        self.stitches.iter().filter(|s| s.kind == kind).count()
    }

    /// Si el diseño, centrado en el origen como lo borda la máquina, cabe en el bastidor.
    pub fn fits_in(&self, hoop: Hoop) -> bool {
        let b = self.bounds();
        hoop.size().is_some_and(|(w, h)| {
            (-b.min_x).max(b.max_x) * 2 <= w && (-b.min_y).max(b.max_y) * 2 <= h
        })
    }

    /// Recalcula las extensiones y, si el diseño ya no cabe, el bastidor.
    /// Úsalo después de modificar las puntadas; los conteos los calcula siempre el escritor.
    pub fn refresh_header(&mut self) {
        let b = self.bounds();
        let (left, top, right, bottom) = (-b.min_x, -b.min_y, b.max_x, b.max_y);
        let margins = |hx: i32, hy: i32| {
            let m = [hx - left, hy - top, hx - right, hy - bottom];
            if m.iter().all(|&v| v >= 0) {
                m
            } else {
                [-1; 4]
            }
        };
        let hoop_b = margins(700, 1000);
        self.header.extents = [
            [left, top, right, bottom],
            margins(630, 550),
            margins(250, 250),
            hoop_b,
            hoop_b,
        ];

        if !self.fits_in(self.header.hoop)
            && let Some(h) = Hoop::BY_SIZE.into_iter().find(|&h| self.fits_in(h))
        {
            self.header.hoop = h;
        }

        self.header
            .thread_types
            .resize(self.colors.len(), crate::DEFAULT_THREAD_TYPE);
    }
}
