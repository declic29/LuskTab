#!/usr/bin/env bash
# Regenerate every app icon from the Lusk Tab logo, assets/app-icon/lusk-tab-logo.png (the logo of
# the Lusk association's spreadsheet, wide, on a transparent background: replace this file to
# change the icon, then run this script).
#
# The logo is centred on a square transparent canvas, assets/app-icon/gridcraft-1024.png, and every
# size is drawn from that square.
#
# Needs: python3 with Pillow (pip install Pillow), and cargo for the Windows .ico. The outputs are
# committed, so builds and packaging never need these tools.
#
#   packaging/icons.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/assets/app-icon"
ID="bzh.lusk.tab"
LOGO="$DIR/lusk-tab-logo.png"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

[ -f "$LOGO" ] || { echo "error: $LOGO not found" >&2; exit 1; }
python3 -c "import PIL" 2>/dev/null || { echo "error: Pillow not found (pip install Pillow)" >&2; exit 1; }

# Windows and Linux use the square full bleed. macOS icons sit on Apple's grid: an 824/1024 body
# with a transparent margin. The .icns is written by Pillow, so iconutil (macOS only) isn't needed.
python3 - "$LOGO" "$DIR" "$ID" "$TMP" <<'EOF'
import os, sys
from PIL import Image

logo_path, out, app_id, tmp = sys.argv[1:5]
logo = Image.open(logo_path).convert("RGBA")
box = logo.getbbox()
if box is None:
    sys.exit(f"error: {logo_path} is empty")
logo = logo.crop(box)

def fit(im, size):
    """im scaled to fit a size x size transparent square, centred."""
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    scale = min(size / im.width, size / im.height)
    w, h = max(1, round(im.width * scale)), max(1, round(im.height * scale))
    canvas.alpha_composite(im.resize((w, h), Image.Resampling.LANCZOS), ((size - w) // 2, (size - h) // 2))
    return canvas

def sized(im, s):
    return im if im.size == (s, s) else im.resize((s, s), Image.Resampling.LANCZOS)

master = fit(logo, 1024)
master.save(os.path.join(out, "gridcraft-1024.png"), optimize=True)

# Apple grid: the square scaled to 824/1024, centred on a transparent 1024 canvas.
mac = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
mac.alpha_composite(sized(master, 824), (100, 100))

# Runtime window/Dock icon on macOS (embedded by apps/gridcraft); other platforms use hicolor 256.
sized(mac, 512).save(os.path.join(out, "gridcraft-macos-512.png"), optimize=True)

# Linux hicolor theme.
for s in (16, 24, 32, 48, 64, 128, 256, 512):
    d = os.path.join(out, "hicolor", f"{s}x{s}", "apps")
    os.makedirs(d, exist_ok=True)
    sized(master, s).save(os.path.join(d, f"{app_id}.png"), optimize=True)

# Windows .ico sizes, packed by xtask below.
for s in (16, 20, 24, 32, 40, 48, 64, 128, 256):
    sized(master, s).save(os.path.join(tmp, f"ico-{s}.png"))

# macOS .icns.
mac.save(os.path.join(out, "gridcraft.icns"), format="ICNS",
         append_images=[sized(mac, s) for s in (16, 32, 64, 128, 256, 512)])
EOF

ICO_PNGS=()
for s in 16 20 24 32 40 48 64 128 256; do
  ICO_PNGS+=("$TMP/ico-$s.png")
done
(cd "$ROOT" && cargo run -q -p xtask --no-default-features -- ico "$DIR/gridcraft.ico" "${ICO_PNGS[@]}")
echo "icons written to $DIR"
