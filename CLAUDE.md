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

- `Objeto { nombre, visible, color (paleta Janome), forma, puntada, opacidad, bloqueado }`.
  `opacidad` solo sirve para ver las guías en el editor (se omite en el JSON si es
  `OPACIDAD_GUIA` = 0,15). `bloqueado` (solo se escribe si es `true`): no se elige en el lienzo.
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
- `fondo` (opcional, se omite si es `None`): `{ruta, centro [mm], ancho mm, opacidad, visible,
  bloqueado}` (`bloqueado` solo se escribe si es `true`). Imagen para calcar; no se borda. Ruta
  relativa = desde la carpeta del `.edenbord`.
- `Generado.rangos[i]` = entradas de `jef.stitches` del objeto `i` (vista previa al arrastrar).
- Portapapeles: `copiar_objetos`/`pegar_objetos` = JSON `{"formato":"edenbord-objetos",
  "objetos":[…]}` en el portapapeles del sistema (sirve entre ventanas).
- `geom::subtrazados` corta un trazado en cada `M` (para "Separar trazados").
- `booleanas.rs` (crate `i_overlay`): `operar(Booleana::{Unir,Restar,Intersecar,Excluir}, a, b)`
  con regla par-impar; aplana las curvas (`geom::TOLERANCIA`), el resultado es solo rectas.
  Varias zonas = `operar_varias` (de a pares, en orden).
- `Puntada::copiar_cambios(antes, despues)`: copia solo los parámetros que cambiaron (edición
  de varios objetos sin pisar lo distinto de cada uno).
- `.ai`: aún no; exportar a SVG desde Illustrator.

## App (`crates/edenbord/src`)

- `main.rs`: estado `Editor`, distribución (`App::ui`), panel derecho de JEF (`side_panel`),
  atajos, diálogos (paleta, confirmaciones), guardado.
- Distribución (`barras.rs`): barra superior (logo, nombre, abrir/nuevo/guardar, deshacer,
  importar; a la derecha la acción principal verde y el menú ☰), columna de herramientas a la
  izquierda (`tools_bar`, `set_tool`), panel derecho (lista arriba en un `Panel::top`
  redimensionable, propiedades abajo) y barra de estado (simulación, vista, mensaje,
  coordenadas). Sin documento: `welcome`. Las barras reservan primero lo de la derecha
  (`right_to_left`) y recortan lo que no cabe. No hay barra de menús (decisión del usuario).
- `contexto.rs`: barra contextual bajo la superior, según `Tool`: Seleccionar = X/Y/ancho/alto
  editables (proyecto: `transform_grouped`, candado proporcional `lock_ratio`; JEF: solo X/Y con
  `Doc::edit_grouped`, el tamaño se muestra), girar/espejar/escalar %/centrar (`Op`), paso de las
  flechas, y con varios objetos combinar, separar y booleanas; Nodos, Pluma y Regla muestran sus
  acciones y la medida. Los paneles ya no tienen "Transformar" ni las secciones de pluma/nodos.
- `lista.rs`: filas de las listas (colores JEF y objetos): `fila` (clic elige, arrastrar
  reordena con `DragAndDrop`; botones de la derecha con prioridad), `clic`, `hueco`, `soltar`.
- `doc.rs`: `Doc` = diseño + visibilidad/selección por bloque + historial (100 copias del `Jef`).
  Ediciones con `doc.edit(|j| …)`; el arrastre con Shift usa `checkpoint()` una vez y luego
  `translate` directo.
- `canvas.rs`: dibujo (común: `draw_stitches`, `Vista`), zoom/desplazamiento. Vista: botón
  central/derecho o Espacio + arrastrar (en JEF también el arrastre principal; ahí la selección
  se mueve con Shift + arrastrar). En proyectos (`select_input`, prioridad en `pick`): manijas →
  objeto (clic elige, arrastrar mueve; Ctrl/Shift agrega; `object_at`: primero bordes, si no la zona
  más chica que contiene el punto; ocultos y bloqueados no se eligen, `elegible`) → interior del
  marco (sin manijas si la selección tiene un bloqueado) → fondo libre →
  vacío = rectángulo de selección (`objects_in`, formas enteras dentro).
- `gestos.rs`: `Editor.gesto` (`Transformar {blanco: Objetos|Fondo, tipo: Mover|Escalar(sx,sy)|
  Girar, r0, desde, af}` o `Caja`). Durante el gesto solo se acumula `af` y se dibuja la vista
  previa (contornos y puntadas ya generadas pasan por `af`); al soltar, un solo `edit`
  (`transform_affine` / `edit_fondo`). Esc cancela. Esquinas proporcionales (Shift libre,
  Ctrl desde el centro), lados un eje, giro con Shift de a 15°, mover con Ctrl recto. El fondo
  solo tiene esquinas (ancho + centro). Funciones puras con tests (`afin_escala`, …).
- `nodos.rs` + `Tool::Nodes`: el `BezPath` pasa a `Sub { nodos: [Nodo {p, entrada, salida}],
  cerrado }` (todo cúbico; manija = punto ⇒ recto) y vuelve con `a_trazado` (rectas como `L`,
  cierre recto implícito en `Z`). Arrastrar nodos: `checkpoint` al empezar,
  `set_nodes_live` (solo polilíneas) mientras, `sync` al soltar. Doble clic = insertar,
  Supr = borrar (mínimo 3 cerrado / 2 abierto), "Esquina ↔ curva". Textos: "Convertir en
  trazado".
- Atajos (`shortcuts`, `clipboard_shortcuts`; no corren si un campo tiene el foco): S/N/P/R
  herramientas, Ctrl+Z/Y, Ctrl+C/X/V (egui los entrega como `Event::Copy/Cut/Paste`; pega en el
  puntero), Ctrl+Shift+V (en el mismo lugar, vía `ViewportCommand::RequestPaste`), Ctrl+D,
  Ctrl+A, Esc, Supr (objetos, nodos o el fondo elegido), flechas.
- Hay a lo sumo un documento abierto: `Editor.doc` (JEF) **o** `Editor.proyecto`. Abrir decide
  por extensión (`.jef`, `.edenbord`/`.bordado`, `.svg` → proyecto nuevo con guías).
- `proyecto.rs`: `ProyectoDoc` = proyecto + selección + historial (copias del `Proyecto` con
  su selección, que vuelve al deshacer) + puntadas generadas y polilíneas en caché (se
  recalculan en `sync()` tras cada edición). `edit_grouped(clave, …)` junta en un solo deshacer
  las ediciones continuas (arrastrar valores); `logic` llama a `end_group()` cuando no hay botón
  apretado ni campo con foco. Con la pluma, Ctrl+Z quita el último vértice.
- `pluma.rs` + `Editor.tool` (`Select`/`Nodes`/`Pen`/`Ruler`): clic = vértice esquina, arrastrar = vértice con
  manija (simétrica, control = punto + manija), clic en el primero = zona, Enter = línea,
  Retroceso o Ctrl+Z/Esc. Con la pluma el arrastre principal no mueve la vista.
  Los trazos nuevos entran como guías. "Combinar zonas" (`ProyectoDoc::combine`) une contornos
  en uno: los de adentro quedan como agujeros. "Separar trazados" (`split`) es lo inverso: un
  objeto por subtrazado, en el mismo lugar del orden. Booleanas (`boolean`): Restar recorta el
  primero seleccionado (el que se borda antes, el fondo) con los demás, **que se conservan**;
  Unir/Intersecar/Excluir dejan el resultado en el primero y borran los demás.
- Guías que son zonas: se ven rellenas (color de hilo × `opacidad`, par-impar) con una malla de
  trapecios (`proyecto::trapecios`, en caché en `ProyectoDoc.relleno_guias`, recalculada en
  `sync()`).
- Lista de objetos y de colores JEF: arrastrar una fila cambia el orden de bordado
  (`lista.rs`); clic derecho = menú (duplicar/borrar, subir/bajar). Ojo y candado en la fila.
- Con varios objetos seleccionados, el panel edita los parámetros del primero y los aplica con
  `copiar_cambios` a los del mismo tipo; el selector de color pinta toda la selección si el
  objeto elegido es parte de ella.
- `tool_drags()`: con nodos, pluma o regla el arrastre principal es de la herramienta, no
  mueve vista ni selección. `overlays` (regla y puntero) lo llaman ambos lienzos.
- Zoom limitado a `MIN_SCALE..=MAX_SCALE` (máx. 60 px/mm): las polilíneas en caché usan
  `geom::TOLERANCIA` (0,02 mm) y con más zoom las curvas se ven angulosas. La vista previa de la
  pluma se aproxima según el zoom.
- Ajuste de la vista: tras ajustar, mientras no se mueva la vista (`fit_size`), si el lienzo
  cambia de tamaño se vuelve a ajustar (la ventana se acomoda después del primer cuadro).
- Cuadrícula (`draw_grid`): alineada al origen, paso adaptativo 1/5/10/50/100 mm (≥ 8 px),
  líneas fuertes cada 10 mm (o 5 pasos), ejes más oscuros. Regla: `Editor.ruler` (mm),
  Shift = múltiplos de 45° (`snap45`), Esc borra; sirve en JEF y proyectos.
- Fondo: textura en `Editor.fondo_tex` (crate `image`), recargada en `logic` si cambia la ruta.
- `panel_proyecto.rs`: guardar `.edenbord` (relee y compara), exportar JEF (`write_verified`),
  importar SVG, agregar texto, panel derecho del proyecto (objetos, propiedades/puntada, fondo). Transformar usa el mismo
  `Op` que los JEF (`apply_jef` / `apply_proyecto`); en proyectos escalar re-densifica solo.
- Guardar es solo "Guardar como…" (sugiere `<nombre>_editado.jef`), relee para verificar y pide
  confirmación antes de sobrescribir el archivo abierto.
- `progress == usize::MAX` significa "mostrar todo" (para que la simulación no corte diseños
  que crecen al editar).
- `tema.rs`: tema claro "lino" (paleta del icono: copihue = acento, hoja = acción principal,
  `AVISO` dorado oscuro), Inter + Phosphor incrustadas (`assets/fuentes/`, ver su `LEEME.md`:
  Inter va recortada sin el área de uso privado, si no tapa los íconos). Helpers
  `boton_icono[_si]`, `interruptor` (ícono Fill sobre copihue si está activo),
  `boton_principal`. `iconos.rs`: constantes Phosphor usadas (agregar acá las nuevas, códigos
  del crate `egui-phosphor`, que no tiene versión para egui 0.37). No usar emojis en la UI.
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

Calcar: imagen de fondo + pluma + combinar zonas. Edición directa: manijas (escalar/girar),
rectángulo de selección, portapapeles, duplicar, editar nodos.

Ideas pendientes:
- `.ai` directo (leyendo su parte PDF).
- Satín automático en formas cerradas y letras (hoy el satín es de ancho fijo sobre el trazado).
- Kerning en textos; marcar cortes de hilo (hoy solo saltos).
- Exportar a DST/EXP (simples) y PES (complejo).
- Soporte `.JBF`.
- Re-densificar al escalar (requiere reconocer rellenos/satines; difícil).
