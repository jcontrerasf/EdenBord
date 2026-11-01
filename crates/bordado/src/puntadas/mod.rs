//! Generadores de puntadas. Todos trabajan en mm y devuelven tramos: secuencias de puntos que
//! se cosen seguidos. Entre un tramo y el siguiente la máquina salta (con remates).

mod column;
mod concentrico;
mod corrido;
mod lineas;
mod motivo;
mod relleno;
mod remate;
mod satin;
mod split;

use kurbo::Point;

pub use column::{AreaSatin, Guide, Layout, area_satin, auto_guides, column_order, columns};
pub use concentrico::concentrico;
pub use corrido::corrido;
pub use lineas::{feston, manual, punto_atras, tallo};
pub use motivo::motivo;
pub use relleno::{Tatami, relleno};
pub use remate::con_remates;
pub use satin::satin;
pub use split::auto_cuts;

/// Puntos que se cosen sin saltar.
pub type Tramo = Vec<Point>;
