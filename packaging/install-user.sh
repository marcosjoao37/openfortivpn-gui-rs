#!/usr/bin/env bash
# User-level install/uninstall of openfortivpn-gui (no root required).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="openfortivpn-gui"
BIN="$ROOT/dist/$PROJECT"
PREFIX="$HOME/.local"

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$PREFIX/bin/$PROJECT" "$PREFIX/share/applications/$PROJECT.desktop"
  rm -f "$PREFIX/share/icons/hicolor/scalable/apps/$PROJECT.svg"
  for s in 16 22 32 48 64 128 256; do
    rm -f "$PREFIX/share/icons/hicolor/${s}x${s}/apps/$PROJECT.png"
  done
  update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
  echo "Uninstalled."
  exit 0
fi

[ -x "$BIN" ] || { echo "error: dist/$PROJECT missing — run 'make build' first" >&2; exit 1; }

install -Dm755 "$BIN" "$PREFIX/bin/$PROJECT"
install -Dm644 "$ROOT/assets/icon.svg" "$PREFIX/share/icons/hicolor/scalable/apps/$PROJECT.svg"
for s in 16 22 32 48 64 128 256; do
  install -Dm644 "$ROOT/assets/icons/$PROJECT-$s.png" \
    "$PREFIX/share/icons/hicolor/${s}x${s}/apps/$PROJECT.png"
done
mkdir -p "$PREFIX/share/applications"
sed "s|@BIN@|$PREFIX/bin/$PROJECT|" "$ROOT/packaging/$PROJECT.desktop.in" \
  > "$PREFIX/share/applications/$PROJECT.desktop"

update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true

MISSING="$(ldd "$PREFIX/bin/$PROJECT" | awk '/not found/{print $1}' | sort -u || true)"
if [ -n "$MISSING" ]; then
  echo "warning: missing runtime libraries: $MISSING" >&2
  echo "hint (Arch/CachyOS): sudo pacman -S gtk3 libayatana-appindicator libxkbcommon-x11" >&2
fi

echo "Installed for user:"
echo "  binary:   $PREFIX/bin/$PROJECT"
echo "  desktop:  $PREFIX/share/applications/$PROJECT.desktop"
echo "  icons:    $PREFIX/share/icons/hicolor/"
echo "Launch from your app menu ('openfortivpn GUI') or run: $PREFIX/bin/$PROJECT"