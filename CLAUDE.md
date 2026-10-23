# EdenBord — editor de bordados JEF (Janome)

Nombre: "Eden" por el himno de Chile ("y tu campo de flores bordado es la copia feliz del
Edén"), "Bord" por bordado. Licencia GPL-3.0-or-later. Versión única en `[workspace.package]`.

Workspace Rust con tres crates:

- `crates/jef` — librería sin UI: lectura/escritura JEF, paleta Janome, operaciones de edición.
- `crates/bordado` — librería sin UI: proyectos editables `.edenbord` (formas + tipo de puntada),
  generación de puntadas, importar SVG, texto con fuentes TTF/OTF.
- `crates/edenbord` — app egui/eframe 0.37 (binario `edenbord`) que usa ambas.

Código, comentarios, mensajes de error e interfaz están **en español**. Mantenerlo así.

## Comandos

```bash
cargo test --workspace                          # incluye pruebas con archivos reales (ver abajo)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo run -p edenbord -- archivo.jef
cargo run -p jef --example info -- a.jef b.jef  # resumen en terminal
cargo run -p bordado --example generar -- dibujo.svg salida.jef vista.svg  # SVG/.edenbord → JEF + vista
```

## Archivos reales de prueba

- `crates/jef/tests/real_files.rs` lee los `.jef` de la carpeta en la variable `JEF_FIXTURES`.
  Sin la variable las pruebas se saltan (con aviso); con una ruta inválida fallan.
- La variable se define en `.cargo/config.toml` (`[env]`), **ignorado por git**. Plantilla:
  `.cargo/config.example.toml`.
- **Nunca** commitear rutas personales (ni como valor por defecto ni en comentarios) ni diseños
  (`*.jef`, `*.JEF`, `*.JBF` están en `.gitignore`). Antes de commitear:
  `git grep --cached -i "/home/" -- ':!CLAUDE.md'` debe salir vacío.

## Formato JEF (verificado con archivos reales)

Little-endian, sin compresión. Unidades de 0,1 mm.

| Offset | Campo |
|---|---|
| 0 | i32 offset al inicio de las puntadas (= 116 + 8·colores, más `gap` si lo hubiera) |
| 4 | i32 flags (`0x0A` en todos los archivos vistos) |
| 8 | 16 bytes fecha ASCII `YYYYMMDDHHMMSS` + 2 de relleno (a veces todo ceros) |
| 24 | i32 nº de colores |
| 28 | i32 nº de "puntadas" = bytes de datos / 2 (incluye comandos y el fin) |
| 32 | i32 bastidor: 0=A 110×110, 1=C 50×50, 2=B 140×200, 3=F 126×110, 4=SQ 200×200 |
| 36 | 5 bloques × 4 i32: bloque 0 = distancias del origen a izq/arriba/der/abajo (incluye el origen); bloques 1–4 = márgenes a bastidores F, C, B y (repetido) B; `-1` = no cabe |
| 116 | i32 por color: índice a la paleta Janome (`palette.rs`, 1..=78) |
| … | i32 por color: tipo de hilo (siempre 13) |

Puntadas: pares `(dx, dy)` i8; **y del archivo crece hacia arriba**, el modelo usa y hacia abajo.
Rango ±127 (−128 = `0x80` choca con el escape). Comandos de 4 bytes `0x80 código dx dy`:
`0x01` cambio de color (siempre `dx,dy = 0`), `0x02` salto, `0x80 0x10` fin (2 bytes).

Rarezas encontradas:
- Hay datos basura después del fin en algunos archivos → se guardan en `Jef::trailing`.
- Archivos viejos (≈2011–2012) tienen los márgenes de bastidor mal calculados → al leer se
  conservan tal cual; solo `refresh_header()` los recalcula (fórmula de los archivos nuevos).
- Saltos de desplazamiento (0,0) aparecen a menudo; probablemente son marcas de corte de hilo.
  **No fusionar ni eliminar saltos.**
- El bastidor declarado no siempre es el mínimo; `refresh_header()` solo lo cambia si no cabe.
- `.JBF` (cabecera `JANOMEB2`) es otro formato de Janome, aún sin soporte.

## Invariantes (tienen tests; no romperlas)

- `write(read(bytes)) == bytes` byte a byte para todos los archivos reales. Por eso `Header`
  guarda todo lo no derivable; el escritor solo calcula offset y conteos.
- `set_blocks(blocks())` es identidad byte a byte en los archivos reales.
- Las ediciones conservan los tramos cosidos (ver helper `sewn` en los tests).
- Editar repetidamente no acumula saltos.

## Modelo y edición (`crates/jef/src`)

- `Stitch { x, y, kind }` en coordenadas **absolutas**; `StitchKind = Normal | Jump | ColorChange`.
  El fin no se guarda; lo agrega el escritor. El escritor parte movimientos > 127 en tramos
  iguales y escribe un cambio de color lejano como saltos + cambio `(0,0)`.
- `edit.rs`: toda edición pasa por `Block { color, anchor, stitches }`. `anchor` es desde dónde
  arranca el bloque (origen para el primero, posición del cambio de color para los demás).
  Regla clave: el anchor solo importa si la primera entrada del bloque es `Normal`
  (`sews_from_anchor`). Solo entonces se transforma y `set_blocks` salta hasta él. Esto evita
  coser líneas entre bloques y que se acumulen saltos.
- `selection_bounds` usa solo puntos cosidos (no saltos); las rotaciones de 90° y los espejos
  son enteros exactos alrededor de ese centro.
- En las operaciones, `sel: &[usize]` vacío = todos los bloques. Las operaciones llaman a
  `refresh_header()`; `set_blocks` no lo hace (para la invariante de identidad).

## Proyectos `.edenbord` (`crates/bordado/src`)

Formato propio (no hay estándar abierto): JSON legible, `{"formato":"edenbord","version":1,
"bastidor":<código Hoop>,"objetos":[…]}`. Unidades **mm**, y hacia abajo, origen = centro del
bastidor. El orden de `objetos` es el orden de bordado. Los proyectos antiguos (extensión
`.bordado`, `"formato":"bordado"` = `FORMATO_ANTIGUO`) se siguen leyendo; Ctrl+S sobre uno de
ellos pide guardarlo como `.edenbord`. Usar `bordado::EXTENSION`, no el texto literal.

- `Objeto { nombre, visible, color (paleta Janome), forma, puntada }`.
- `Forma` (`tipo`): `contorno {d}` (cerrada, agujeros por par-impar) | `linea {d}` |
  `texto {texto, fuente (ruta), alto (mayúsculas, mm), espaciado, afin [6]}`. `d` es el `d` de
  SVG (kurbo `BezPath`), guardado con 3 decimales. Transformar un texto compone su `afin`.
- `Puntada` (`tipo`): `ninguna` (guía, no se borda) | `corrido {largo, triple}` |
  `satin {ancho, densidad, base}` | `relleno {angulo, separacion, largo, base, compensacion}`.
  `PRESETS` = la "paleta" de tipos. Relleno solo en zonas; corrido/satín sobre cualquier trazado
  (en zonas, sobre el borde).
- `Proyecto::generar()` → `Generado { jef, objeto_de_bloque, avisos }`: cada objeto da tramos
  (mm); cada tramo lleva remates (`puntadas/remate.rs`) y se une con un salto; objetos seguidos
  del mismo color no cambian de hilo. Termina en `Jef::new` (cabecera estándar, fecha actual en
  UTC) que ya llama a `refresh_header()`.
- Relleno tatami (`puntadas/relleno.rs`): gira la forma, barre filas, agrupa segmentos en
  secciones 1:1 (para no cruzar agujeros) y cose cada sección en serpentina; desfase de ladrillo
  por fila. La base es el mismo relleno a +90°, ralo y metido 0,5 mm.
- `svg.rs`: usvg (transforms, unidades, textos con fuentes del sistema) → objetos guía,
  color = `palette::nearest`, centrados en el origen. 96 px por pulgada.
- `fondo` (opcional, se omite si es `None`): `{ruta, centro [mm], ancho mm, opacidad, visible}`.
  Imagen para calcar; no se borda. Ruta relativa = desde la carpeta del `.edenbord`.
- `.ai`: aún no; exportar a SVG desde Illustrator.

## App (`crates/edenbord/src`)

- `main.rs`: estado `Editor`, panel lateral, atajos, diálogos (paleta, confirmaciones), guardado.
- `doc.rs`: `Doc` = diseño + visibilidad/selección por bloque + historial (100 copias del `Jef`).
  Ediciones con `doc.edit(|j| …)`; el arrastre con Shift usa `checkpoint()` una vez y luego
  `translate` directo.
- `canvas.rs`: dibujo (común: `draw_stitches`, `Vista`), zoom/desplazamiento, arrastre de la
  selección; en proyectos, clic elige objeto (`ProyectoDoc::object_at`).
- Hay a lo sumo un documento abierto: `Editor.doc` (JEF) **o** `Editor.proyecto`. Abrir decide
  por extensión (`.jef`, `.edenbord`/`.bordado`, `.svg` → proyecto nuevo con guías).
- `proyecto.rs`: `ProyectoDoc` = proyecto + selección + historial (copias del `Proyecto`) +
  puntadas generadas y polilíneas en caché (se recalculan en `sync()` tras cada edición).
  `edit_grouped(clave, …)` junta en un solo deshacer las ediciones continuas (arrastrar valores).
- `pluma.rs` + `Editor.tool` (`Select`/`Pen`): clic = vértice esquina, arrastrar = vértice con
  manija (simétrica, control = punto + manija), clic en el primero = zona, Enter = línea,
  Retroceso/Esc. Con la pluma el arrastre principal no mueve la vista (rueda/botón central sí).
  Los trazos nuevos entran como guías. "Combinar zonas" (`ProyectoDoc::combine`) une contornos
  en uno: los de adentro quedan como agujeros.
- Barra sobre el lienzo (`canvas.rs::toolbar`): herramienta (`Tool::Select/Pen/Ruler`),
  cuadrícula y posición del puntero en mm. `tool_drags()`: con pluma o regla el arrastre
  principal es de la herramienta, no mueve vista ni selección.
- Cuadrícula (`draw_grid`): alineada al origen, paso adaptativo 1/5/10/50/100 mm (≥ 8 px),
  líneas fuertes cada 10 mm (o 5 pasos), ejes más oscuros. Regla: `Editor.ruler` (mm),
  Shift = múltiplos de 45° (`snap45`), Esc borra; sirve en JEF y proyectos.
- Fondo: textura en `Editor.fondo_tex` (crate `image`), recargada en `logic` si cambia la ruta.
- `panel_proyecto.rs`: guardar `.edenbord` (relee y compara), exportar JEF (`write_verified`),
  importar SVG, agregar texto, lista de objetos, propiedades/puntada. Transformar usa el mismo
  `Op` que los JEF (`apply_jef` / `apply_proyecto`); en proyectos escalar re-densifica solo.
- Guardar es solo "Guardar como…" (sugiere `<nombre>_editado.jef`), relee para verificar y pide
  confirmación antes de sobrescribir el archivo abierto.
- `progress == usize::MAX` significa "mostrar todo" (para que la simulación no corte diseños
  que crecen al editar).
- API de egui 0.37 (distinta a versiones viejas): `App::ui(&mut self, ui, frame)` + `App::logic`;
  `egui::Panel::left(..).default_size(..).show(ui, ..)`; `CentralPanel::show(ui, ..)`;
  `Window::show(ctx, ..)`; `DroppedFile` es un trait (`f.path()`).

## Empaquetado y versiones

- `assets/edenbord.svg` es el icono; `packaging/iconos.sh` regenera los PNG y el `.ico`
  (Inkscape + ImageMagick). La ventana usa `edenbord-256.png` (`include_bytes!`), el `.exe` el
  `.ico` (`build.rs` con `winresource`, solo al compilar en Windows).
- `app_id` = `edenbord` = nombre del `.desktop`, del icono instalado y `StartupWMClass`.
- `.deb`: `cargo deb -p edenbord` (metadatos en `crates/edenbord/Cargo.toml`); instala
  `.desktop`, iconos hicolor y tipos MIME (`packaging/linux/`).
- AppImage: `packaging/appimage.sh [versión]` (necesita `appimagetool`, o la variable
  `APPIMAGETOOL`). Salida en `target/paquetes/`.
- Windows: `packaging/windows/edenbord.iss` (Inno Setup; asocia `.edenbord` y opcionalmente
  `.jef`) + zip portable. Solo se arma en CI.
- `.github/workflows/ci.yml`: fmt, clippy, tests (Linux) y build en Windows.
  `release.yml`: con tag `vX.Y.Z` (debe coincidir con la versión de Cargo.toml) arma .deb,
  AppImage, instalador y zip y publica el Release; `workflow_dispatch` solo deja artefactos.
- Sacar versión: subir `version` en `[workspace.package]`, commit, `git tag vX.Y.Z`,
  `git push --tags`.

## Estado y próximos pasos

Hecho: lectura/escritura, visor, edición básica (mover, rotar, espejar, escalar, centrar,
recolorear, reordenar, borrar, combinar, deshacer, guardar como). Proyectos `.edenbord`:
importar SVG, convertir trazados en relleno/satín/pespunte, texto TTF, exportar JEF.

Calcar: imagen de fondo + pluma + combinar zonas.

Ideas pendientes:
- Editar nodos de un trazo ya creado (hoy solo se redibuja); mover/escalar el fondo con el ratón.
- `.ai` directo (leyendo su parte PDF).
- Satín automático en formas cerradas y letras (hoy el satín es de ancho fijo sobre el trazado).
- Kerning en textos; marcar cortes de hilo (hoy solo saltos).
- Exportar a DST/EXP (simples) y PES (complejo).
- Soporte `.JBF`.
- Re-densificar al escalar (requiere reconocer rellenos/satines; difícil).
- Mover un bloque a mano sigue sin poder separarse de su orden de bordado (solo ⏶/⏷).
