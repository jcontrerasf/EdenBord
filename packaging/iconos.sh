#!/usr/bin/env bash
# Regenera los PNG y el ICO de assets/ a partir de assets/edenbord.svg.
# Requiere Inkscape e ImageMagick.
set -euo pipefail
cd "$(dirname "$0")/../assets"
for n in 16 32 48 64 128 256 512; do
    inkscape edenbord.svg --export-type=png --export-width="$n" --export-filename="edenbord-$n.png" 2>/dev/null
done
magick edenbord-16.png edenbord-32.png edenbord-48.png edenbord-64.png edenbord-128.png \
    edenbord-256.png edenbord.ico
