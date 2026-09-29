#!/usr/bin/env bash
# Renders all PNG icons from assets/icon.svg via rsvg-convert.
# Tray variants recolor the __ACCENT__ placeholder.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v rsvg-convert >/dev/null || { echo "rsvg-convert not found" >&2; exit 1; }

mkdir -p assets/icons tmp-icons

# App icon (brand accent)
sed 's/__ACCENT__/#2f6fed/g' assets/icon.svg > tmp-icons/app.svg
for s in 16 22 32 48 64 128 256; do
  rsvg-convert -w "$s" -h "$s" tmp-icons/app.svg -o "assets/icons/openfortivpn-gui-$s.png"
done

# Tray variants (32 px)
tray() {
  sed "s/__ACCENT__/$2/g" assets/icon.svg > "tmp-icons/$1.svg"
  rsvg-convert -w 32 -h 32 "tmp-icons/$1.svg" -o "assets/icons/tray-$1-32.png"
}
tray idle       '#6b7280'
tray connecting '#f59e0b'
tray connected  '#22c55e'
tray error      '#dc2626'

rm -rf tmp-icons
ls -1 assets/icons