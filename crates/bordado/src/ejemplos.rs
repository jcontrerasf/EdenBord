//! Proyectos de ejemplo, armados en código (así siguen al día con los tipos de puntada).

use kurbo::{Affine, BezPath, Circle, Ellipse, Point, RoundedRect, Shape};

use crate::fuentes::{self, Fuente};
use crate::proyecto::{Forma, Lado, Motivo, OPACIDAD_GUIA, Objeto, PRESETS, Proyecto, Puntada};
use crate::{Grupo, geom, preset_por_nombre, texto};

pub struct Ejemplo {
    pub nombre: &'static str,
    pub descripcion: &'static str,
    pub crear: fn() -> Proyecto,
}

pub const EJEMPLOS: &[Ejemplo] = &[
    Ejemplo {
        nombre: "Muestrario de puntadas",
        descripcion: "Todos los tipos de puntada, uno al lado del otro y con su nombre",
        crear: muestrario,
    },
    Ejemplo {
        nombre: "Flor",
        descripcion: "Pétalos con relleno en distintos ángulos, centro concéntrico, tallo de \
                      satín con puntas finas y hojas con nervio",
        crear: flor,
    },
    Ejemplo {
        nombre: "Marco con texto",
        descripcion: "Borde de festón, onda decorativa y un texto rellenado",
        crear: marco,
    },
];

/// Color de la paleta Janome más parecido.
fn color(rgb: [u8; 3]) -> i32 {
    jef::palette::nearest(rgb)
}

fn objeto(nombre: &str, color: i32, forma: Forma, puntada: Puntada) -> Objeto {
    Objeto {
        nombre: nombre.into(),
        visible: true,
        color,
        forma,
        puntada,
        opacidad: OPACIDAD_GUIA,
        bloqueado: false,
    }
}

fn zona(d: BezPath) -> Forma {
    Forma::Contorno { d }
}

fn linea(d: &str) -> Forma {
    Forma::Linea {
        d: BezPath::from_svg(d).expect("trazado de ejemplo válido"),
    }
}

/// Texto centrado en `centro` (mm); `None` si no hay fuentes en el sistema.
fn texto_centrado(
    t: &str,
    fuente: &Fuente,
    alto: f64,
    centro: Point,
    color: i32,
    puntada: Puntada,
) -> Option<Objeto> {
    let caja = texto::trazado(t, fuente, alto, 0.0).ok()?.bounding_box();
    // Redondeado a 0,01 mm: el JSON queda limpio y se relee igual.
    let d = centro - caja.center();
    let afin = Affine::translate(((d.x * 100.0).round() / 100.0, (d.y * 100.0).round() / 100.0));
    Some(objeto(
        t,
        color,
        Forma::Texto {
            texto: t.into(),
            fuente: fuente.clone(),
            alto,
            espaciado: 0.0,
            afin: afin.as_coeffs(),
        },
        puntada,
    ))
}

/// Grilla con todos los presets sobre una curva o una zona con agujero, y su nombre como guía.
fn muestrario() -> Proyecto {
    const PASO: f64 = 38.0;
    const COLUMNAS: usize = 5;
    let colores = [
        [200, 30, 60],
        [40, 90, 170],
        [60, 130, 60],
        [230, 150, 30],
        [120, 60, 150],
        [20, 130, 140],
    ]
    .map(color);
    let fuente = fuentes::predeterminada();
    let filas = PRESETS.len().div_ceil(COLUMNAS);
    let mut objetos = Vec::new();
    for (i, p) in PRESETS.iter().enumerate() {
        let (c, f) = (i % COLUMNAS, i / COLUMNAS);
        let centro = Point::new(
            (c as f64 - (COLUMNAS - 1) as f64 / 2.0) * PASO,
            (f as f64 - (filas - 1) as f64 / 2.0) * PASO - 3.0,
        );
        let d = if p.grupo == Grupo::Relleno {
            "M-14,1 C-14,-12 14,-12 14,1 L14,10 L-14,10 Z M-4,-1 L4,-1 L4,6 L-4,6 Z"
        } else {
            "M-14,7 C-7,-14 7,18 14,-7"
        };
        let mut d = BezPath::from_svg(d).expect("forma de muestra válida");
        d.apply_affine(Affine::translate(centro.to_vec2()));
        let forma = if p.grupo == Grupo::Relleno {
            zona(d)
        } else {
            Forma::Linea { d }
        };
        objetos.push(objeto(
            p.nombre,
            colores[i % colores.len()],
            forma,
            p.puntada.clone(),
        ));
        // El nombre, como guía (no se borda), bien visible y bloqueado: un clic elige la muestra.
        if let Some(f) = &fuente
            && let Some(mut o) = texto_centrado(
                p.nombre,
                f,
                2.5,
                centro + kurbo::Vec2::new(0.0, 16.0),
                color([60, 50, 40]),
                Puntada::Ninguna,
            )
        {
            o.opacidad = 0.6;
            o.bloqueado = true;
            objetos.push(o);
        }
    }
    Proyecto {
        bastidor: jef::Hoop::Sq200x200.code(),
        objetos,
        ..Default::default()
    }
}

fn flor() -> Proyecto {
    let rosado = color([225, 90, 140]);
    let rosado_oscuro = color([160, 30, 80]);
    let amarillo = color([245, 200, 40]);
    let verde = color([70, 140, 60]);
    let verde_oscuro = color([30, 80, 40]);
    let centro = Point::new(0.0, -18.0);
    let mut objetos = Vec::new();

    // Primero lo de atrás: tallo y hojas (los pétalos van encima).
    objetos.push(objeto(
        "tallo",
        verde_oscuro,
        linea("M0,0 C-4,14 4,28 0,44"),
        preset_por_nombre("Satín ahusado"),
    ));
    for (nombre, hoja, nervio, angulo) in [
        (
            "hoja izquierda",
            "M0,24 C-8,12 -22,12 -27,6 C-22,20 -10,28 0,24 Z",
            "M0,24 C-9,18 -18,12 -27,6",
            -25.0,
        ),
        (
            "hoja derecha",
            "M0,32 C8,20 22,20 27,14 C22,28 10,36 0,32 Z",
            "M0,32 C9,26 18,20 27,14",
            25.0,
        ),
    ] {
        let mut p = preset_por_nombre("Relleno liviano");
        if let Some(a) = p.angulo_mut() {
            *a = angulo;
        }
        let d = BezPath::from_svg(hoja).expect("hoja válida");
        objetos.push(objeto(nombre, verde, zona(d), p));
        objetos.push(objeto(
            &format!("nervio {nombre}"),
            verde_oscuro,
            linea(nervio),
            preset_por_nombre("Punto tallo"),
        ));
    }
    // Pétalos: cada uno con las filas a lo largo, desde el centro hacia afuera.
    let mut bordes = BezPath::new();
    for k in 0..6 {
        let giro = f64::from(k) * 60.0;
        let eje = Affine::rotate_about(giro.to_radians(), centro);
        let petalo = eje * Ellipse::new(centro + kurbo::Vec2::new(0.0, -13.0), (5.5, 10.0), 0.0);
        let d = petalo.to_path(geom::TOLERANCIA);
        bordes.extend(d.iter());
        let mut p = preset_por_nombre("Relleno 45°");
        if let Some(a) = p.angulo_mut() {
            *a = giro + 90.0;
        }
        objetos.push(objeto(&format!("pétalo {}", k + 1), rosado, zona(d), p));
    }
    objetos.push(objeto(
        "borde de los pétalos",
        rosado_oscuro,
        zona(bordes),
        Puntada::Corrido {
            largo: 2.0,
            pasadas: 1,
        },
    ));
    objetos.push(objeto(
        "centro",
        amarillo,
        zona(Circle::new(centro, 6.0).to_path(geom::TOLERANCIA)),
        preset_por_nombre("Concéntrico"),
    ));

    objetos.push(objeto(
        "pasto",
        verde,
        linea("M-35,47 L35,47"),
        Puntada::Motivo {
            motivo: Motivo::Sierra,
            largo: 3.0,
            ancho: 4.0,
        },
    ));
    Proyecto {
        bastidor: jef::Hoop::A110x110.code(),
        objetos,
        ..Default::default()
    }
}

fn marco() -> Proyecto {
    let rojo = color([200, 30, 60]);
    let azul = color([40, 70, 140]);
    let mut objetos = vec![
        objeto(
            "borde",
            azul,
            zona(RoundedRect::new(-45.0, -30.0, 45.0, 30.0, 8.0).to_path(geom::TOLERANCIA)),
            Puntada::Feston {
                ancho: 3.0,
                separacion: 2.5,
                lado: Lado::Izquierda,
            },
        ),
        objeto(
            "onda",
            rojo,
            zona(RoundedRect::new(-38.0, -23.0, 38.0, 23.0, 5.0).to_path(geom::TOLERANCIA)),
            Puntada::Motivo {
                motivo: Motivo::Onda,
                largo: 6.0,
                ancho: 2.5,
            },
        ),
    ];
    if let Some(f) = fuentes::predeterminada() {
        let negrita = Fuente::sistema(f.familia.as_deref().unwrap_or(""), fuentes::NEGRITA, false);
        let f = if negrita.familia.is_some() && texto::trazado("E", &negrita, 1.0, 0.0).is_ok() {
            negrita
        } else {
            f
        };
        objetos.extend(texto_centrado(
            "EdenBord",
            &f,
            11.0,
            Point::ZERO,
            rojo,
            preset_por_nombre("Relleno horizontal"),
        ));
    }
    Proyecto {
        bastidor: jef::Hoop::A110x110.code(),
        objetos,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_generate_inside_their_hoop() {
        for e in EJEMPLOS {
            let p = (e.crear)();
            let g = p.generar();
            assert!(g.avisos.is_empty(), "{}: {:?}", e.nombre, g.avisos);
            assert!(g.jef.count(jef::StitchKind::Normal) > 500, "{}", e.nombre);
            // El diseño cabe en el bastidor del proyecto (con algo de margen).
            let (ancho, alto) = jef::Hoop::from_code(p.bastidor).size().unwrap();
            let b = g.jef.bounds();
            assert!(
                b.width() <= ancho - 40 && b.height() <= alto - 40,
                "{}: {}×{} en {ancho}×{alto}",
                e.nombre,
                b.width(),
                b.height()
            );
            // Se puede guardar y volver a leer (el JSON redondea a 3 decimales).
            let json = p.guardar();
            assert_eq!(Proyecto::leer(&json).unwrap().guardar(), json);
        }
    }
}
