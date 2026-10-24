//! Aspecto de la aplicación: paleta "lino" (tomada del icono), Inter y los íconos de Phosphor.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Response,
    RichText, Shadow, Stroke, TextStyle, Theme, Ui, Vec2,
};

// Paleta.
/// Fondo de los paneles: lino crudo.
pub const LINO: Color32 = Color32::from_rgb(0xf6, 0xef, 0xe0);
/// Ventanas, campos de texto y fondos "hundidos".
pub const LINO_CLARO: Color32 = Color32::from_rgb(0xfc, 0xf9, 0xf2);
/// Botones en reposo.
const ARENA: Color32 = Color32::from_rgb(0xec, 0xe2, 0xcd);
const ARENA_HOVER: Color32 = Color32::from_rgb(0xe3, 0xd5, 0xb9);
const ARENA_ACTIVA: Color32 = Color32::from_rgb(0xd6, 0xc4, 0xa2);
/// Bordes y separadores.
pub const BORDE: Color32 = Color32::from_rgb(0xd9, 0xcb, 0xae);
/// Texto: café oscuro, como hilo de algodón.
pub const TEXTO: Color32 = Color32::from_rgb(0x3b, 0x2f, 0x25);
pub const TEXTO_SUAVE: Color32 = Color32::from_rgb(0x7a, 0x68, 0x55);
/// Acento: rojo copihue (selección, herramienta activa, foco).
pub const COPIHUE: Color32 = Color32::from_rgb(0xd7, 0x26, 0x3d);
const COPIHUE_OSCURO: Color32 = Color32::from_rgb(0x9e, 0x16, 0x2a);
const COPIHUE_SUAVE: Color32 = Color32::from_rgb(0xf5, 0xcf, 0xd3);
/// Acción principal (exportar, guardar): verde hoja.
pub const HOJA: Color32 = Color32::from_rgb(0x3d, 0x7a, 0x34);
/// Avisos: el dorado del icono, oscurecido para que se lea sobre el lino.
pub const AVISO: Color32 = Color32::from_rgb(0xa8, 0x6b, 0x00);

/// Familia con los íconos rellenos (para lo que está activo).
pub const FILL: &str = "fill";
/// Familia con Inter SemiBold (títulos).
pub const FUERTE: &str = "fuerte";

/// Tamaño de los íconos en las barras de herramientas.
const TAM_ICONO: f32 = 18.0;

pub fn instalar(ctx: &egui::Context) {
    ctx.set_fonts(fuentes());
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.set_visuals_of(Theme::Light, visuales());
    ctx.style_mut_of(Theme::Light, |s| {
        s.text_styles = [
            (TextStyle::Small, FontId::proportional(11.0)),
            (TextStyle::Body, FontId::proportional(13.5)),
            (TextStyle::Button, FontId::proportional(13.5)),
            (TextStyle::Monospace, FontId::monospace(12.5)),
            (
                TextStyle::Heading,
                FontId::new(17.0, FontFamily::Name(FUERTE.into())),
            ),
        ]
        .into();
        let sp = &mut s.spacing;
        sp.item_spacing = Vec2::new(8.0, 6.0);
        sp.button_padding = Vec2::new(8.0, 4.0);
        sp.interact_size.y = 24.0;
        sp.window_margin = Margin::same(12);
        sp.menu_margin = Margin::same(6);
        sp.slider_width = 140.0;
        sp.indent = 16.0;
        s.interaction.tooltip_delay = 0.35;
    });
}

fn fuentes() -> FontDefinitions {
    let mut f = FontDefinitions::default();
    let datos: [(&str, &'static [u8]); 4] = [
        (
            "inter",
            include_bytes!("../assets/fuentes/Inter-Regular.ttf"),
        ),
        (
            "inter-semibold",
            include_bytes!("../assets/fuentes/Inter-SemiBold.ttf"),
        ),
        ("phosphor", include_bytes!("../assets/fuentes/Phosphor.ttf")),
        (
            "phosphor-fill",
            include_bytes!("../assets/fuentes/Phosphor-Fill.ttf"),
        ),
    ];
    for (nombre, bytes) in datos {
        f.font_data
            .insert(nombre.into(), FontData::from_static(bytes).into());
    }
    // Las de egui quedan de respaldo (símbolos y emojis que Inter no tiene).
    let respaldo = f.families[&FontFamily::Proportional].clone();
    let familia = |texto: &str, iconos: &str| {
        let mut v = vec![texto.to_string(), iconos.to_string()];
        v.extend(respaldo.iter().cloned());
        v
    };
    f.families
        .insert(FontFamily::Proportional, familia("inter", "phosphor"));
    f.families.insert(
        FontFamily::Name(FILL.into()),
        familia("inter", "phosphor-fill"),
    );
    f.families.insert(
        FontFamily::Name(FUERTE.into()),
        familia("inter-semibold", "phosphor"),
    );
    if let Some(mono) = f.families.get_mut(&FontFamily::Monospace) {
        mono.insert(1.min(mono.len()), "phosphor".into());
    }
    f
}

fn visuales() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.override_text_color = None;
    v.panel_fill = LINO;
    v.window_fill = LINO_CLARO;
    v.window_stroke = Stroke::new(1.0, BORDE);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(30),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(25),
    };
    v.faint_bg_color = Color32::from_rgb(0xf0, 0xe7, 0xd4);
    v.extreme_bg_color = LINO_CLARO;
    v.code_bg_color = ARENA;
    v.hyperlink_color = COPIHUE_OSCURO;
    v.warn_fg_color = AVISO;
    v.error_fg_color = COPIHUE;
    v.selection.bg_fill = COPIHUE_SUAVE;
    v.selection.stroke = Stroke::new(1.0, COPIHUE_OSCURO);
    v.text_cursor.stroke = Stroke::new(2.0, COPIHUE);
    v.slider_trailing_fill = true;
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let w = &mut v.widgets;
    let radio = CornerRadius::same(6);
    w.noninteractive.bg_fill = LINO;
    w.noninteractive.weak_bg_fill = LINO;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXTO);
    w.noninteractive.corner_radius = radio;
    for (estado, fondo, texto) in [
        (&mut w.inactive, ARENA, TEXTO),
        (&mut w.hovered, ARENA_HOVER, TEXTO),
        (&mut w.active, ARENA_ACTIVA, TEXTO),
        (&mut w.open, ARENA_HOVER, TEXTO),
    ] {
        estado.bg_fill = fondo;
        estado.weak_bg_fill = fondo;
        estado.fg_stroke = Stroke::new(1.0, texto);
        estado.corner_radius = radio;
        estado.expansion = 0.0;
    }
    w.inactive.bg_stroke = Stroke::NONE;
    w.hovered.bg_stroke = Stroke::new(1.0, BORDE);
    w.active.bg_stroke = Stroke::new(1.0, TEXTO_SUAVE);
    w.open.bg_stroke = Stroke::new(1.0, BORDE);
    v
}

/// Texto de un ícono, relleno si `activo`.
pub fn icono(icono: &str, activo: bool) -> RichText {
    let familia = if activo {
        FontFamily::Name(FILL.into())
    } else {
        FontFamily::Proportional
    };
    RichText::new(icono).family(familia).size(TAM_ICONO)
}

/// Botón cuadrado con solo un ícono; `ayuda` va en el tooltip (nombre y atajo).
pub fn boton_icono(ui: &mut Ui, ico: &str, ayuda: &str) -> Response {
    boton_icono_si(ui, true, ico, ayuda)
}

/// Como [`boton_icono`], deshabilitado si `!enabled`.
pub fn boton_icono_si(ui: &mut Ui, enabled: bool, ico: &str, ayuda: &str) -> Response {
    let b = egui::Button::new(icono(ico, false))
        .frame_when_inactive(false)
        .min_size(Vec2::splat(30.0));
    ui.add_enabled(enabled, b).on_hover_text(ayuda)
}

/// Botón de herramienta o interruptor: ícono relleno sobre el acento si está activo.
pub fn interruptor(ui: &mut Ui, activo: bool, ico: &str, ayuda: &str) -> Response {
    let texto = icono(ico, activo).color(if activo { Color32::WHITE } else { TEXTO });
    let mut b = egui::Button::new(texto).min_size(Vec2::splat(30.0));
    b = if activo {
        b.fill(COPIHUE)
    } else {
        b.frame_when_inactive(false)
    };
    ui.add(b).on_hover_text(ayuda)
}

/// Botón de la acción principal (verde hoja, texto blanco).
pub fn boton_principal(ui: &mut Ui, ico: &str, texto: &str) -> Response {
    let r = egui::Button::new(
        RichText::new(format!("{ico}  {texto}"))
            .color(Color32::WHITE)
            .family(FontFamily::Name(FUERTE.into())),
    )
    .fill(HOJA)
    .min_size(Vec2::new(0.0, 30.0));
    ui.add(r)
}

/// Botón chico de ícono para las filas de las listas: marcado si `activo`, suave si no.
pub fn mini(ui: &mut Ui, activo: bool, ico: &str, ayuda: &str) -> Response {
    let color = if activo {
        TEXTO
    } else {
        TEXTO_SUAVE.gamma_multiply(0.7)
    };
    let texto = RichText::new(ico).size(15.0).color(color);
    ui.add(egui::Button::new(texto).frame_when_inactive(false).small())
        .on_hover_text(ayuda)
}

/// Muestra redonda de un color de hilo; se puede apretar.
pub fn muestra(ui: &mut Ui, color: Color32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
    let borde = if resp.hovered() { TEXTO } else { BORDE };
    ui.painter()
        .circle(rect.center(), 8.0, color, Stroke::new(1.5, borde));
    resp
}

/// Título de sección del panel: chico, en mayúsculas y suave.
pub fn titulo(ui: &mut Ui, texto: &str) {
    ui.label(
        RichText::new(texto.to_uppercase())
            .size(11.0)
            .family(FontFamily::Name(FUERTE.into()))
            .color(TEXTO_SUAVE),
    );
}
