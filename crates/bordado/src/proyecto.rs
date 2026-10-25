//! Modelo del proyecto editable `.edenbord` (JSON).
//!
//! Todo va en milímetros, con y hacia abajo y el origen en el centro del bastidor. Las formas
//! se guardan como el atributo `d` de SVG para que el archivo sea legible y fácil de editar.

use std::path::{Path, PathBuf};

use kurbo::{Affine, BezPath, PathEl};
use serde::{Deserialize, Serialize};

/// Valor del campo `formato`, para reconocer el archivo.
pub const FORMATO: &str = "edenbord";
/// Valor de `formato` en los proyectos anteriores al nombre EdenBord (extensión `.bordado`).
/// Se siguen leyendo; al guardar se escribe [`FORMATO`].
pub const FORMATO_ANTIGUO: &str = "bordado";
/// Extensión de los archivos de proyecto (sin punto).
pub const EXTENSION: &str = "edenbord";
/// Versión del formato que escribe esta librería.
pub const VERSION: u32 = 2;
/// `formato` del texto que se copia al portapapeles (ver [`copiar_objetos`]).
pub const FORMATO_PORTAPAPELES: &str = "edenbord-objetos";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proyecto {
    pub formato: String,
    pub version: u32,
    /// Código de bastidor JEF (ver [`jef::Hoop::code`]).
    pub bastidor: i32,
    /// Imagen para calcar; no se borda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fondo: Option<Fondo>,
    /// Objetos en orden de bordado.
    pub objetos: Vec<Objeto>,
}

/// Imagen de referencia bajo el diseño.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fondo {
    /// Ruta de la imagen; si es relativa, se toma desde la carpeta del proyecto.
    pub ruta: PathBuf,
    /// Centro de la imagen, en mm.
    pub centro: [f64; 2],
    /// Ancho en mm; el alto sale de la proporción de la imagen.
    pub ancho: f64,
    /// 0 = invisible, 1 = opaca.
    pub opacidad: f32,
    #[serde(default = "si")]
    pub visible: bool,
    /// Bloqueada: los clics la atraviesan y no se puede mover ni escalar con el ratón.
    #[serde(default, skip_serializing_if = "no")]
    pub bloqueado: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objeto {
    pub nombre: String,
    #[serde(default = "si")]
    pub visible: bool,
    /// Índice en la paleta Janome.
    pub color: i32,
    pub forma: Forma,
    pub puntada: Puntada,
    /// Opacidad (0–1) del relleno con que se ve una guía en el editor; no afecta al bordado.
    #[serde(default = "opacidad_guia", skip_serializing_if = "es_opacidad_guia")]
    pub opacidad: f32,
    /// Bloqueado: no se elige ni se mueve en el lienzo (sí desde la lista de objetos).
    #[serde(default, skip_serializing_if = "no")]
    pub bloqueado: bool,
}

/// Opacidad con que se ven las guías si no se elige otra: muy traslúcidas.
pub const OPACIDAD_GUIA: f32 = 0.15;

fn opacidad_guia() -> f32 {
    OPACIDAD_GUIA
}

fn es_opacidad_guia(o: &f32) -> bool {
    *o == OPACIDAD_GUIA
}

fn si() -> bool {
    true
}

fn no(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Forma {
    /// Zona cerrada; los subtrazados interiores son agujeros (regla par-impar).
    Contorno {
        #[serde(with = "svg_d")]
        d: BezPath,
    },
    /// Trazado abierto (o varios).
    Linea {
        #[serde(with = "svg_d")]
        d: BezPath,
    },
    /// Texto con una fuente TrueType/OpenType; se convierte en contorno al generar.
    Texto {
        texto: String,
        fuente: PathBuf,
        /// Alto de las mayúsculas, en mm.
        alto: f64,
        /// Espacio extra entre letras, en mm.
        #[serde(default)]
        espaciado: f64,
        /// Posición, giro y escala: lleva el texto (línea base en y = 0, desde x = 0) a su lugar.
        afin: [f64; 6],
    },
}

/// Tipo de puntada y sus parámetros (mm y grados).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Puntada {
    /// Guía: se ve en el editor pero no se borda.
    Ninguna,
    /// Pespunte sobre el trazado (en los contornos, sobre el borde).
    Corrido {
        largo: f64,
        /// Veces que se cose cada puntada (ida, vuelta, ida…): 1 simple, 3 triple, 5 o más =
        /// «bean», una línea gruesa. Siempre impar, para terminar adelante.
        #[serde(default = "una")]
        pasadas: u32,
    },
    /// Una puntada en cada nodo del trazado, tal como se dibujó.
    Manual,
    /// Punto atrás: cada puntada avanza dos pasos y vuelve uno. Línea pareja con aspecto de
    /// cosido a mano.
    PuntoAtras { largo: f64 },
    /// Punto tallo (cordón): puntadas inclinadas que se montan unas sobre otras.
    Tallo { largo: f64, ancho: f64 },
    /// Festón (punto manta, «E»): pespunte con dientes perpendiculares. Para apliqué y bordes.
    Feston {
        ancho: f64,
        /// Distancia entre dientes.
        separacion: f64,
        lado: Lado,
    },
    /// Un dibujo que se repite a lo largo del trazado.
    Motivo {
        motivo: Motivo,
        /// Largo de cada repetición.
        largo: f64,
        ancho: f64,
    },
    /// Columna en zigzag a lo largo del trazado.
    Satin {
        ancho: f64,
        /// Distancia entre dos puntadas seguidas del mismo lado.
        densidad: f64,
        /// Puntadas de abajo que asientan el satín.
        base: BaseSatin,
        /// Cuánto se ensancha cada lado para compensar el tirón del hilo.
        #[serde(default)]
        compensacion: f64,
        /// Puntas finas al empezar y terminar (tallos, hojas, trazos caligráficos).
        #[serde(default)]
        ahusado: bool,
    },
    /// Relleno tatami de una zona cerrada.
    Relleno {
        /// Dirección de las filas; 0 = horizontal, positivo = horario en pantalla.
        angulo: f64,
        /// Distancia entre filas.
        separacion: f64,
        largo: f64,
        /// Relleno ralo cruzado antes del relleno, para que no se hunda en la tela.
        base: bool,
        /// Cuánto se alarga cada fila en sus extremos para compensar el tirón del hilo.
        compensacion: f64,
        /// Cómo se desfasan las puntadas de una fila a la siguiente.
        #[serde(default)]
        patron: Patron,
        /// Degradé: la separación pasa de `separacion` (primera fila) a esta (última).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separacion_final: Option<f64>,
    },
    /// Relleno de filas que siguen el borde de la zona, de afuera hacia adentro.
    Concentrico { separacion: f64, largo: f64 },
    /// Dos rellenos ralos cruzados: cuadrícula decorativa que deja ver la tela.
    Cuadrille {
        angulo: f64,
        separacion: f64,
        largo: f64,
    },
}

fn una() -> u32 {
    1
}

/// Hacia qué lado del trazado (según el sentido en que se dibujó) van los dientes del festón.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lado {
    #[default]
    Izquierda,
    Derecha,
    /// Alternados.
    Ambos,
}

impl Lado {
    pub const TODOS: [Lado; 3] = [Lado::Izquierda, Lado::Derecha, Lado::Ambos];

    pub fn nombre(self) -> &'static str {
        match self {
            Lado::Izquierda => "Izquierda",
            Lado::Derecha => "Derecha",
            Lado::Ambos => "Alternado",
        }
    }
}

/// Dibujos de [`Puntada::Motivo`] (ver `puntadas/motivo.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    #[default]
    Onda,
    Sierra,
    Almenas,
    Rombos,
    Festones,
    Cruces,
}

impl Motivo {
    pub const TODOS: [Motivo; 6] = [
        Motivo::Onda,
        Motivo::Sierra,
        Motivo::Almenas,
        Motivo::Rombos,
        Motivo::Festones,
        Motivo::Cruces,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            Motivo::Onda => "Onda",
            Motivo::Sierra => "Sierra",
            Motivo::Almenas => "Almenas",
            Motivo::Rombos => "Rombos",
            Motivo::Festones => "Festones",
            Motivo::Cruces => "Cruces",
        }
    }
}

/// Base del satín (lo que se cose antes, debajo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseSatin {
    Ninguna,
    /// Pespunte por el centro: para columnas angostas.
    #[default]
    Central,
    /// Pespunte por dentro de los dos bordes: bordes firmes en columnas medianas.
    Contorno,
    /// Zigzag ralo: levanta y sostiene columnas anchas.
    Zigzag,
}

impl BaseSatin {
    pub const TODAS: [BaseSatin; 4] = [
        BaseSatin::Ninguna,
        BaseSatin::Central,
        BaseSatin::Contorno,
        BaseSatin::Zigzag,
    ];

    pub fn nombre(self) -> &'static str {
        match self {
            BaseSatin::Ninguna => "Sin base",
            BaseSatin::Central => "Central",
            BaseSatin::Contorno => "Contorno",
            BaseSatin::Zigzag => "Zigzag",
        }
    }
}

/// Desfase de las puntadas entre filas del relleno: qué dibujo forman los agujeros.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Patron {
    /// Como ladrillos: el más parejo, no se notan líneas.
    #[default]
    Ladrillo,
    /// Las puntadas forman diagonales suaves.
    Diagonal,
    /// Desfase irregular (repetido cada 7 filas): textura sin dibujo.
    Irregular,
}

impl Patron {
    pub const TODOS: [Patron; 3] = [Patron::Ladrillo, Patron::Diagonal, Patron::Irregular];

    pub fn nombre(self) -> &'static str {
        match self {
            Patron::Ladrillo => "Ladrillo",
            Patron::Diagonal => "Diagonal",
            Patron::Irregular => "Irregular",
        }
    }
}

impl Puntada {
    pub fn nombre(&self) -> &'static str {
        match self {
            Puntada::Ninguna => "Guía (no se borda)",
            Puntada::Corrido { pasadas: 1, .. } => "Pespunte",
            Puntada::Corrido { pasadas: 3, .. } => "Pespunte triple",
            Puntada::Corrido { .. } => "Pespunte grueso",
            Puntada::Manual => "Manual",
            Puntada::PuntoAtras { .. } => "Punto atrás",
            Puntada::Tallo { .. } => "Punto tallo",
            Puntada::Feston { .. } => "Festón",
            Puntada::Motivo { .. } => "Motivo",
            Puntada::Satin { .. } => "Satín",
            Puntada::Relleno { .. } => "Relleno",
            Puntada::Concentrico { .. } => "Relleno concéntrico",
            Puntada::Cuadrille { .. } => "Cuadrillé",
        }
    }

    /// Si solo sirve para zonas (rellenos). Las demás siguen el trazado, o el borde de una zona.
    pub fn solo_zonas(&self) -> bool {
        matches!(
            self,
            Puntada::Relleno { .. } | Puntada::Concentrico { .. } | Puntada::Cuadrille { .. }
        )
    }

    /// El `tipo` del JSON: igual para dos puntadas de la misma clase con otros parámetros.
    pub fn tipo(&self) -> std::mem::Discriminant<Puntada> {
        std::mem::discriminant(self)
    }

    /// Copia a `self` solo los parámetros que cambiaron de `antes` a `despues` (los tres del
    /// mismo tipo; si no, no hace nada). Sirve para editar varios objetos a la vez sin pisar lo
    /// que cada uno tiene distinto. Compara los campos del JSON, así sirve para todos los tipos.
    pub fn copiar_cambios(&mut self, antes: &Puntada, despues: &Puntada) {
        use serde_json::Value;
        if self.tipo() != antes.tipo() || antes.tipo() != despues.tipo() {
            return;
        }
        let campos = |p: &Puntada| match serde_json::to_value(p) {
            Ok(Value::Object(m)) => m,
            _ => serde_json::Map::new(),
        };
        let (a, d, mut destino) = (campos(antes), campos(despues), campos(self));
        for (k, v) in &d {
            if a.get(k) != Some(v) {
                destino.insert(k.clone(), v.clone());
            }
        }
        // Campos opcionales que se quitaron (se omiten en el JSON si son `None`).
        for k in a.keys().filter(|k| !d.contains_key(*k)) {
            destino.remove(k);
        }
        if let Ok(p) = serde_json::from_value(Value::Object(destino)) {
            *self = p;
        }
    }
}

/// En qué parte de la paleta va un preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grupo {
    Guia,
    /// Siguen el trazado (o el borde de una zona).
    Linea,
    /// Solo zonas.
    Relleno,
}

/// Un tipo de puntada listo para usar; los parámetros se ajustan después.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub nombre: &'static str,
    /// Para qué sirve (se muestra al pasar el puntero en la paleta).
    pub descripcion: &'static str,
    pub grupo: Grupo,
    pub puntada: Puntada,
}

const fn preset(
    nombre: &'static str,
    descripcion: &'static str,
    grupo: Grupo,
    puntada: Puntada,
) -> Preset {
    Preset {
        nombre,
        descripcion,
        grupo,
        puntada,
    }
}

const fn satin(ancho: f64, densidad: f64, base: BaseSatin, ahusado: bool) -> Puntada {
    Puntada::Satin {
        ancho,
        densidad,
        base,
        compensacion: 0.15,
        ahusado,
    }
}

const fn relleno(angulo: f64, separacion: f64, largo: f64, base: bool) -> Puntada {
    Puntada::Relleno {
        angulo,
        separacion,
        largo,
        base,
        compensacion: 0.2,
        patron: Patron::Ladrillo,
        separacion_final: None,
    }
}

/// La paleta de tipos de puntada, en el orden en que se muestran.
pub const PRESETS: &[Preset] = &[
    preset(
        "Guía",
        "Se ve en el editor pero no se borda: para calcar o como referencia",
        Grupo::Guia,
        Puntada::Ninguna,
    ),
    preset(
        "Pespunte",
        "Línea fina de puntadas seguidas: detalles, contornos delicados",
        Grupo::Linea,
        Puntada::Corrido {
            largo: 2.5,
            pasadas: 1,
        },
    ),
    preset(
        "Pespunte triple",
        "Cada puntada se cose tres veces: contornos marcados",
        Grupo::Linea,
        Puntada::Corrido {
            largo: 2.5,
            pasadas: 3,
        },
    ),
    preset(
        "Pespunte grueso",
        "Cinco pasadas por puntada («bean»): línea gruesa y en relieve",
        Grupo::Linea,
        Puntada::Corrido {
            largo: 2.5,
            pasadas: 5,
        },
    ),
    preset(
        "Manual",
        "Una puntada en cada nodo, tal como se dibujó con la pluma",
        Grupo::Linea,
        Puntada::Manual,
    ),
    preset(
        "Punto atrás",
        "Avanza dos pasos y vuelve uno: línea pareja con aspecto de cosido a mano",
        Grupo::Linea,
        Puntada::PuntoAtras { largo: 2.0 },
    ),
    preset(
        "Punto tallo",
        "Puntadas inclinadas que se montan: cordón para tallos, ramas y letras manuscritas",
        Grupo::Linea,
        Puntada::Tallo {
            largo: 3.0,
            ancho: 0.8,
        },
    ),
    preset(
        "Festón",
        "Pespunte con dientes hacia un lado (punto manta): bordes de apliqué",
        Grupo::Linea,
        Puntada::Feston {
            ancho: 3.0,
            separacion: 2.0,
            lado: Lado::Izquierda,
        },
    ),
    preset(
        "Zigzag",
        "Zigzag abierto: une bordes o decora sin tapar la tela",
        Grupo::Linea,
        Puntada::Satin {
            ancho: 2.5,
            densidad: 2.0,
            base: BaseSatin::Ninguna,
            compensacion: 0.0,
            ahusado: false,
        },
    ),
    preset(
        "Satín 2 mm",
        "Columna brillante y tupida: contornos gruesos, letras finas",
        Grupo::Linea,
        satin(2.0, 0.4, BaseSatin::Central, false),
    ),
    preset(
        "Satín 4 mm",
        "Columna ancha con base de contorno",
        Grupo::Linea,
        satin(4.0, 0.4, BaseSatin::Contorno, false),
    ),
    preset(
        "Satín ahusado",
        "Satín con puntas finas: tallos, hojas, trazos caligráficos",
        Grupo::Linea,
        satin(3.0, 0.4, BaseSatin::Central, true),
    ),
    preset(
        "Onda",
        "Línea ondulada que se repite a lo largo del trazado",
        Grupo::Linea,
        Puntada::Motivo {
            motivo: Motivo::Onda,
            largo: 5.0,
            ancho: 2.5,
        },
    ),
    preset(
        "Sierra",
        "Dientes en zigzag abiertos",
        Grupo::Linea,
        Puntada::Motivo {
            motivo: Motivo::Sierra,
            largo: 4.0,
            ancho: 3.0,
        },
    ),
    preset(
        "Rombos",
        "Cadena de rombos",
        Grupo::Linea,
        Puntada::Motivo {
            motivo: Motivo::Rombos,
            largo: 4.0,
            ancho: 3.0,
        },
    ),
    preset(
        "Punto cruz",
        "Fila de cruces, como el punto cruz a mano",
        Grupo::Linea,
        Puntada::Motivo {
            motivo: Motivo::Cruces,
            largo: 3.0,
            ancho: 3.0,
        },
    ),
    preset(
        "Relleno 45°",
        "Tatami: filas parejas que cubren la zona",
        Grupo::Relleno,
        relleno(45.0, 0.4, 3.5, true),
    ),
    preset(
        "Relleno horizontal",
        "Tatami con filas horizontales",
        Grupo::Relleno,
        relleno(0.0, 0.4, 3.5, true),
    ),
    preset(
        "Relleno liviano",
        "Tatami ralo: menos puntadas, la tela se nota un poco",
        Grupo::Relleno,
        Puntada::Relleno {
            angulo: 45.0,
            separacion: 0.7,
            largo: 4.0,
            base: false,
            compensacion: 0.1,
            patron: Patron::Ladrillo,
            separacion_final: None,
        },
    ),
    preset(
        "Degradé",
        "Las filas se van separando: el color se desvanece hacia un lado",
        Grupo::Relleno,
        Puntada::Relleno {
            angulo: 90.0,
            separacion: 0.4,
            largo: 3.5,
            base: false,
            compensacion: 0.1,
            patron: Patron::Irregular,
            separacion_final: Some(1.6),
        },
    ),
    preset(
        "Concéntrico",
        "Filas que siguen el borde de la zona, de afuera hacia adentro",
        Grupo::Relleno,
        Puntada::Concentrico {
            separacion: 0.5,
            largo: 2.5,
        },
    ),
    preset(
        "Cuadrillé",
        "Cuadrícula de pespunte: relleno decorativo y liviano",
        Grupo::Relleno,
        Puntada::Cuadrille {
            angulo: 45.0,
            separacion: 3.0,
            largo: 3.0,
        },
    ),
];

/// La puntada de un preset por nombre (`Ninguna` si no existe).
pub fn preset_por_nombre(nombre: &str) -> Puntada {
    PRESETS
        .iter()
        .find(|p| p.nombre == nombre)
        .map_or(Puntada::Ninguna, |p| p.puntada.clone())
}

#[derive(Debug, thiserror::Error)]
pub enum ProyectoError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no es un proyecto de bordado (formato «{0}»)")]
    Formato(String),
    #[error("versión {0} no soportada (esta versión lee hasta la {VERSION})")]
    Version(u32),
}

impl Default for Proyecto {
    fn default() -> Self {
        Proyecto {
            formato: FORMATO.into(),
            version: VERSION,
            bastidor: jef::Hoop::A110x110.code(),
            fondo: None,
            objetos: vec![],
        }
    }
}

impl Proyecto {
    pub fn leer(texto: &str) -> Result<Self, ProyectoError> {
        let mut valor: serde_json::Value = serde_json::from_str(texto)?;
        migrar(&mut valor);
        let mut p: Proyecto = serde_json::from_value(valor)?;
        if p.formato != FORMATO && p.formato != FORMATO_ANTIGUO {
            return Err(ProyectoError::Formato(p.formato));
        }
        p.formato = FORMATO.into();
        if p.version > VERSION {
            return Err(ProyectoError::Version(p.version));
        }
        p.version = VERSION;
        Ok(p)
    }

    pub fn guardar(&self) -> String {
        // Solo falla con mapas de claves no textuales, que el modelo no tiene.
        serde_json::to_string_pretty(self).expect("el proyecto siempre se puede serializar")
    }

    pub fn leer_archivo(path: impl AsRef<Path>) -> Result<Self, ProyectoError> {
        Self::leer(&std::fs::read_to_string(path)?)
    }
}

impl Objeto {
    /// Aplica una transformación (en mm) a la forma.
    pub fn transformar(&mut self, t: Affine) {
        match &mut self.forma {
            Forma::Contorno { d } | Forma::Linea { d } => d.apply_affine(t),
            Forma::Texto { afin, .. } => *afin = (t * Affine::new(*afin)).as_coeffs(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Portapapeles {
    formato: String,
    objetos: Vec<Objeto>,
}

/// Objetos como texto JSON para el portapapeles del sistema (así se pueden pegar en otra
/// ventana): `{"formato":"edenbord-objetos","objetos":[…]}`.
pub fn copiar_objetos(objetos: &[Objeto]) -> String {
    serde_json::to_string(&Portapapeles {
        formato: FORMATO_PORTAPAPELES.into(),
        objetos: objetos.to_vec(),
    })
    .expect("los objetos siempre se pueden serializar")
}

/// Lee lo que dejó [`copiar_objetos`]; `None` si el texto es otra cosa.
pub fn pegar_objetos(texto: &str) -> Option<Vec<Objeto>> {
    let mut valor: serde_json::Value = serde_json::from_str(texto.trim()).ok()?;
    migrar(&mut valor);
    let p: Portapapeles = serde_json::from_value(valor).ok()?;
    (p.formato == FORMATO_PORTAPAPELES).then_some(p.objetos)
}

/// Pasa los campos de la versión 1 a la actual, sobre el JSON (`{"objetos": […]}`, de un
/// proyecto o del portapapeles). Mira la forma de cada campo, no el número de versión, así
/// que aplicarla dos veces no cambia nada.
fn migrar(valor: &mut serde_json::Value) {
    use serde_json::Value;
    let Some(objetos) = valor.get_mut("objetos").and_then(Value::as_array_mut) else {
        return;
    };
    for o in objetos {
        let Some(p) = o.get_mut("puntada").and_then(Value::as_object_mut) else {
            continue;
        };
        // v1: `corrido {triple: bool}` → `pasadas`.
        if let Some(Value::Bool(triple)) = p.remove("triple") {
            p.insert("pasadas".into(), (if triple { 3 } else { 1 }).into());
        }
        // v1: `satin {base: bool}` → `ninguna` / `central`.
        if p.get("tipo").and_then(Value::as_str) == Some("satin")
            && let Some(Value::Bool(b)) = p.get("base")
        {
            let base = if *b { "central" } else { "ninguna" };
            p.insert("base".into(), base.into());
        }
    }
}

/// (De)serializa un [`BezPath`] como texto `d` de SVG, con coordenadas redondeadas a µm.
mod svg_d {
    use std::fmt::Write;

    use super::*;

    pub fn serialize<S: serde::Serializer>(path: &BezPath, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&to_svg(path))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BezPath, D::Error> {
        let text = String::deserialize(d)?;
        BezPath::from_svg(&text).map_err(serde::de::Error::custom)
    }

    fn to_svg(path: &BezPath) -> String {
        let mut out = String::new();
        let mut put = |cmd: &str, pts: &[kurbo::Point]| {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(cmd);
            for (i, p) in pts.iter().enumerate() {
                let sep = if i == 0 { "" } else { " " };
                let _ = write!(out, "{sep}{},{}", num(p.x), num(p.y));
            }
        };
        for el in path.elements() {
            match *el {
                PathEl::MoveTo(p) => put("M", &[p]),
                PathEl::LineTo(p) => put("L", &[p]),
                PathEl::QuadTo(a, b) => put("Q", &[a, b]),
                PathEl::CurveTo(a, b, c) => put("C", &[a, b, c]),
                PathEl::ClosePath => put("Z", &[]),
            }
        }
        out
    }

    fn num(v: f64) -> String {
        let r = (v * 1000.0).round() / 1000.0;
        // Evita "-0".
        format!("{}", if r == 0.0 { 0.0 } else { r })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_changes_keeps_other_fields() {
        let relleno = |angulo, separacion| Puntada::Relleno {
            angulo,
            separacion,
            largo: 4.0,
            base: true,
            compensacion: 0.2,
            patron: Patron::Ladrillo,
            separacion_final: None,
        };
        // Solo cambió la separación: el ángulo propio del destino se conserva.
        let mut destino = relleno(45.0, 0.4);
        destino.copiar_cambios(&relleno(0.0, 0.4), &relleno(0.0, 0.6));
        assert_eq!(destino, relleno(45.0, 0.6));

        // Otro tipo: no se toca.
        let mut satin = preset_por_nombre("Satín 2 mm");
        satin.copiar_cambios(&relleno(0.0, 0.4), &relleno(0.0, 0.6));
        assert_eq!(satin, preset_por_nombre("Satín 2 mm"));
    }

    #[test]
    fn json_roundtrip() {
        let mut p = Proyecto::default();
        p.objetos.push(Objeto {
            nombre: "cuadrado".into(),
            visible: true,
            color: 10,
            forma: Forma::Contorno {
                d: BezPath::from_svg("M0,0 L10.5,0 L10.5,-3.25 Q5,2 0,0 Z").unwrap(),
            },
            puntada: preset_por_nombre("Relleno 45°"),
            opacidad: OPACIDAD_GUIA,
            bloqueado: false,
        });
        p.objetos.push(Objeto {
            nombre: "texto".into(),
            visible: false,
            color: 1,
            forma: Forma::Texto {
                texto: "Hola".into(),
                fuente: "fuente.ttf".into(),
                alto: 8.0,
                espaciado: 0.5,
                afin: Affine::translate((1.0, 2.0)).as_coeffs(),
            },
            puntada: Puntada::Ninguna,
            opacidad: OPACIDAD_GUIA,
            bloqueado: false,
        });
        let text = p.guardar();
        assert!(!text.contains("fondo"), "sin fondo no se escribe el campo");
        p.fondo = Some(Fondo {
            ruta: "foto.png".into(),
            centro: [1.5, -2.0],
            ancho: 80.0,
            opacidad: 0.5,
            visible: true,
            bloqueado: false,
        });
        let text = p.guardar();
        assert!(
            !text.contains("bloqueado"),
            "solo se escribe si está bloqueado"
        );
        assert!(text.contains("\"d\": \"M0,0 L10.5,0 L10.5,-3.25 Q5,2 0,0 Z\""));
        let back = Proyecto::leer(&text).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.guardar(), text);

        p.fondo.as_mut().unwrap().bloqueado = true;
        let text = p.guardar();
        assert!(text.contains("\"bloqueado\": true"));
        assert_eq!(Proyecto::leer(&text).unwrap(), p);

        p.fondo.as_mut().unwrap().bloqueado = false;
        p.objetos[0].bloqueado = true;
        let text = p.guardar();
        assert_eq!(
            text.matches("bloqueado").count(),
            1,
            "solo el objeto bloqueado"
        );
        assert_eq!(Proyecto::leer(&text).unwrap(), p);
    }

    #[test]
    fn clipboard_roundtrip() {
        let o = Objeto {
            nombre: "zona".into(),
            visible: true,
            color: 3,
            forma: Forma::Linea {
                d: BezPath::from_svg("M0,0 L1.5,2").unwrap(),
            },
            puntada: preset_por_nombre("Pespunte"),
            opacidad: OPACIDAD_GUIA,
            bloqueado: false,
        };
        let text = copiar_objetos(std::slice::from_ref(&o));
        assert_eq!(pegar_objetos(&text), Some(vec![o]));
        assert_eq!(pegar_objetos("hola"), None);
        assert_eq!(pegar_objetos(r#"{"formato":"otro","objetos":[]}"#), None);
    }

    #[test]
    fn copy_changes_works_for_every_type() {
        let feston = |ancho, lado| Puntada::Feston {
            ancho,
            separacion: 2.0,
            lado,
        };
        let mut destino = feston(5.0, Lado::Ambos);
        destino.copiar_cambios(&feston(3.0, Lado::Izquierda), &feston(3.0, Lado::Derecha));
        assert_eq!(destino, feston(5.0, Lado::Derecha));

        // Quitar el degradé (campo opcional) también se copia.
        let degrade = preset_por_nombre("Degradé");
        let mut sin = degrade.clone();
        if let Puntada::Relleno {
            separacion_final, ..
        } = &mut sin
        {
            *separacion_final = None;
        }
        let mut destino = degrade.clone();
        destino.copiar_cambios(&degrade, &sin);
        assert_eq!(destino, sin);
    }

    #[test]
    fn presets_roundtrip_and_have_unique_names() {
        let mut nombres: Vec<_> = PRESETS.iter().map(|p| p.nombre).collect();
        nombres.sort();
        nombres.dedup();
        assert_eq!(nombres.len(), PRESETS.len());
        for p in PRESETS {
            let texto = serde_json::to_string(&p.puntada).unwrap();
            assert_eq!(serde_json::from_str::<Puntada>(&texto).unwrap(), p.puntada);
            assert_eq!(
                p.grupo == Grupo::Relleno,
                p.puntada.solo_zonas(),
                "{}",
                p.nombre
            );
        }
    }

    #[test]
    fn migrates_version_1_stitches() {
        let objeto = |puntada: &str| {
            format!(
                r#"{{"nombre":"x","color":1,"forma":{{"tipo":"linea","d":"M0,0 L1,1"}},"puntada":{puntada}}}"#
            )
        };
        let texto = format!(
            r#"{{"formato":"edenbord","version":1,"bastidor":0,"objetos":[{},{},{},{}]}}"#,
            objeto(r#"{"tipo":"corrido","largo":2.5,"triple":true}"#),
            objeto(r#"{"tipo":"corrido","largo":2.0,"triple":false}"#),
            objeto(r#"{"tipo":"satin","ancho":3.0,"densidad":0.4,"base":true}"#),
            objeto(
                r#"{"tipo":"relleno","angulo":45.0,"separacion":0.4,"largo":3.5,"base":false,"compensacion":0.2}"#
            ),
        );
        let p = Proyecto::leer(&texto).unwrap();
        assert_eq!(p.version, VERSION);
        let puntadas: Vec<_> = p.objetos.iter().map(|o| o.puntada.clone()).collect();
        assert_eq!(
            puntadas,
            vec![
                Puntada::Corrido {
                    largo: 2.5,
                    pasadas: 3
                },
                Puntada::Corrido {
                    largo: 2.0,
                    pasadas: 1
                },
                Puntada::Satin {
                    ancho: 3.0,
                    densidad: 0.4,
                    base: BaseSatin::Central,
                    compensacion: 0.0,
                    ahusado: false,
                },
                Puntada::Relleno {
                    angulo: 45.0,
                    separacion: 0.4,
                    largo: 3.5,
                    base: false,
                    compensacion: 0.2,
                    patron: Patron::Ladrillo,
                    separacion_final: None,
                },
            ]
        );
        // Releer lo guardado no cambia nada (la migración no se aplica dos veces).
        assert_eq!(Proyecto::leer(&p.guardar()).unwrap(), p);
        let pegado = format!(
            r#"{{"formato":"edenbord-objetos","objetos":[{}]}}"#,
            objeto(r#"{"tipo":"satin","ancho":3.0,"densidad":0.4,"base":false}"#)
        );
        assert!(matches!(
            pegar_objetos(&pegado).unwrap()[0].puntada,
            Puntada::Satin {
                base: BaseSatin::Ninguna,
                ..
            }
        ));
    }

    #[test]
    fn reads_old_format() {
        let text = r#"{"formato":"bordado","version":1,"bastidor":0,"objetos":[]}"#;
        let p = Proyecto::leer(text).unwrap();
        assert_eq!(p, Proyecto::default());
        assert!(p.guardar().contains(r#""formato": "edenbord""#));
    }

    #[test]
    fn rejects_other_formats() {
        let text = r#"{"formato":"otro","version":1,"bastidor":0,"objetos":[]}"#;
        assert!(matches!(
            Proyecto::leer(text),
            Err(ProyectoError::Formato(_))
        ));
        let text = r#"{"formato":"edenbord","version":99,"bastidor":0,"objetos":[]}"#;
        assert!(matches!(
            Proyecto::leer(text),
            Err(ProyectoError::Version(99))
        ));
    }
}
