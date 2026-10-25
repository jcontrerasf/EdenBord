//! Generadores de puntadas. Todos trabajan en mm y devuelven tramos: secuencias de puntos que
//! se cosen seguidos. Entre un tramo y el siguiente la máquina salta (con remates).

mod concentrico;
mod corrido;
mod lineas;
mod motivo;
mod relleno;
mod remate;
mod satin;

use kurbo::Point;

pub use concentrico::concentrico;
pub use corrido::corrido;
pub use lineas::{feston, manual, punto_atras, tallo};
pub use motivo::motivo;
pub use relleno::{Tatami, relleno};
pub use remate::con_remates;
pub use satin::satin;

/// Puntos que se cosen sin saltar.
pub type Tramo = Vec<Point>;
