# EdenBord

Editor de bordados para máquinas **Janome** (archivos `.jef`), hecho en Rust.

> *«…y tu campo de flores bordado es la copia feliz del Edén.»* — Himno Nacional de Chile
>
> **Eden** viene de esa estrofa; **Bord**, de bordado.

## Qué hace

- Abre, visualiza y simula archivos JEF (puntada a puntada).
- Edita diseños JEF: mover, rotar, espejar, escalar, centrar, recolorear, reordenar, borrar y
  combinar, con deshacer.
- Crea bordados nuevos como proyectos `.edenbord`: importar SVG, dibujar con la pluma sobre una
  imagen de fondo, texto con fuentes TTF/OTF, y convertir cada forma en relleno, satín o
  pespunte. Luego se exporta a JEF para la máquina.

## Descargas

En [Releases](../../releases) hay:

| Sistema | Archivo |
|---|---|
| Linux (Debian, Ubuntu, Mint…) | `edenbord_<versión>_amd64.deb` |
| Linux (cualquiera) | `EdenBord-<versión>-x86_64.AppImage` (darle permiso de ejecución) |
| Windows | `EdenBord-<versión>-instalador.exe` o el `.zip` portable |

## Compilar

```bash
cargo run --release -p edenbord -- [archivo.jef | proyecto.edenbord | dibujo.svg]
```

Paquetes: `cargo deb -p edenbord` y `packaging/appimage.sh` (ver `CLAUDE.md`).

## Licencia

GPL-3.0 o posterior. Ver [LICENSE](LICENSE).
