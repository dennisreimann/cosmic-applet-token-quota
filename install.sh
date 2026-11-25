#!/usr/bin/env bash
# Installs the cosmic-applet-token-quota applet user-locally into ~/.local.
# Usage: ./install.sh   (or override $PREFIX/BINDIR/DATADIR)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ID="net.d11n.CosmicTokenQuota"
DESKTOP_SRC="$HERE/net.d11n.CosmicTokenQuota.desktop"
[ -f "$DESKTOP_SRC" ] || DESKTOP_SRC="$HERE/data/net.d11n.CosmicTokenQuota.desktop"
ICON_SRC="$HERE/assets/o.svg"
BIN_SRC="$HERE/cosmic-applet-token-quota"
SETTINGS_SRC="$HERE/cosmic-applet-token-quota-settings"
if [ ! -x "$BIN_SRC" ]; then
    BIN_SRC="$HERE/target/release/cosmic-applet-token-quota"
fi
if [ ! -x "$SETTINGS_SRC" ]; then
    SETTINGS_SRC="$HERE/target/release/cosmic-applet-token-quota-settings"
fi

BINDIR="${BINDIR:-$HOME/.local/bin}"
DATADIR="${DATADIR:-$HOME/.local/share}"

for f in "$BIN_SRC" "$SETTINGS_SRC" "$DESKTOP_SRC" "$ICON_SRC"; do
    [ -f "$f" ] || { echo "Error: $f is missing (run from the extracted tar.gz)" >&2; exit 1; }
done

install -Dm0755 "$BIN_SRC" "$BINDIR/cosmic-applet-token-quota"
install -Dm0755 "$SETTINGS_SRC" "$BINDIR/cosmic-applet-token-quota-settings"
install -Dm0644 "$ICON_SRC" "$DATADIR/icons/hicolor/scalable/apps/$APP_ID.svg"

DESKTOP_DST="$DATADIR/applications/$APP_ID.desktop"
mkdir -p "$DATADIR/applications"
sed "s|@BIN@|$BINDIR/cosmic-applet-token-quota|" "$DESKTOP_SRC" > "$DESKTOP_DST"

echo "Installed:"
echo "  $BINDIR/cosmic-applet-token-quota"
echo "  $BINDIR/cosmic-applet-token-quota-settings"
echo "  $DESKTOP_DST"
echo "  $DATADIR/icons/hicolor/scalable/apps/$APP_ID.svg"
echo
echo "Add the 'Token Quota' applet in Settings → Panel → Applets (right wing)."
echo "API keys (per provider) go in the applet's own Settings window —"
echo "open the applet popup → Settings. Supported: OpenCode Go, Z.ai Coding Plan."
