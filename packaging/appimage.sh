#!/usr/bin/env bash
# Arma EdenBord-<versión>-x86_64.AppImage en target/paquetes/.
# Uso: packaging/appimage.sh [versión]   (compila en release si hace falta)
# Necesita `appimagetool` en el PATH o en la variable APPIMAGETOOL.
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:-$(cargo pkgid -p edenbord | sed 's/.*[#@]//')}"
tool="${APPIMAGETOOL:-appimagetool}"

cargo build --release -p edenbord
app=target/AppDir
rm -rf "$app"
mkdir -p "$app/usr/bin" "$app/usr/share/applications" "$app/usr/share/mime/packages" \
    "$app/usr/share/icons/hicolor/256x256/apps"
cp target/release/edenbord "$app/usr/bin/"
cp packaging/linux/edenbord.desktop "$app/usr/share/applications/"
cp packaging/linux/edenbord-mime.xml "$app/usr/share/mime/packages/edenbord.xml"
cp assets/edenbord-256.png "$app/usr/share/icons/hicolor/256x256/apps/edenbord.png"
# appimagetool espera el .desktop y el icono también en la raíz.
cp packaging/linux/edenbord.desktop "$app/"
cp assets/edenbord-256.png "$app/edenbord.png"
cp assets/edenbord-256.png "$app/.DirIcon"
cat > "$app/AppRun" <<'RUN'
#!/bin/sh
here="$(dirname "$(readlink -f "$0")")"
exec "$here/usr/bin/edenbord" "$@"
RUN
chmod +x "$app/AppRun"

mkdir -p target/paquetes
ARCH=x86_64 "$tool" --no-appstream "$app" "target/paquetes/EdenBord-$version-x86_64.AppImage"
