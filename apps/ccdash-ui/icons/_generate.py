#!/usr/bin/env python3
"""Generate ccdash icons.

The glyph is a stylized terminal-cursor block: a filled rounded square in
the accent color with a smaller block in the top-right corner suggesting a
dashboard/window/cursor split. Dark background. Designed for round +
square corner clipping.

Outputs:
  - 1024x1024 master at icons/icon.png
  - sized PNGs: 32, 64, 128, 256, 512
  - .icns (built via `iconutil` on macOS — separate shell step)
"""

import os
from PIL import Image, ImageDraw

OUT_DIR = os.path.dirname(os.path.abspath(__file__))

BG = (26, 27, 30, 255)        # --bg
ACCENT = (122, 162, 247, 255) # --accent dark theme
INK = (37, 38, 43, 255)       # --bg-elev (cursor inset)

def render(size: int) -> Image.Image:
    """Render the icon at `size` x `size`. Vector-style: all dimensions
    derived from size so it stays crisp across resolutions."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # Rounded-rect background "tile" with deep-dark color.
    pad = int(size * 0.06)
    radius = int(size * 0.22)
    d.rounded_rectangle(
        [pad, pad, size - pad, size - pad],
        radius=radius,
        fill=BG,
    )

    # Inner accent block (centered, ~60% size, suggesting a window).
    margin = int(size * 0.20)
    inner_radius = int(size * 0.07)
    d.rounded_rectangle(
        [margin, margin, size - margin, size - margin],
        radius=inner_radius,
        fill=ACCENT,
    )

    # Cursor block inset (top-right quadrant, ~16% size).
    cursor_size = int(size * 0.20)
    cx = size - margin - cursor_size - int(size * 0.04)
    cy = margin + int(size * 0.04)
    d.rounded_rectangle(
        [cx, cy, cx + cursor_size, cy + cursor_size],
        radius=int(size * 0.03),
        fill=INK,
    )

    # Subtle bottom "dock" bar to anchor the dashboard feel.
    bar_h = int(size * 0.04)
    bar_pad = int(size * 0.30)
    d.rounded_rectangle(
        [bar_pad, size - margin - bar_h - int(size * 0.05), size - bar_pad, size - margin - int(size * 0.05)],
        radius=int(bar_h / 2),
        fill=INK,
    )

    return img

def main():
    master = render(1024)
    master.save(os.path.join(OUT_DIR, "icon.png"))
    for size in (32, 64, 128, 256, 512):
        master.resize((size, size), Image.LANCZOS).save(
            os.path.join(OUT_DIR, f"{size}x{size}.png")
        )
    # macOS retina @2x variants for .iconset:
    iconset_dir = os.path.join(OUT_DIR, "AppIcon.iconset")
    os.makedirs(iconset_dir, exist_ok=True)
    sizes = [(16, "16x16"), (32, "16x16@2x"), (32, "32x32"),
             (64, "32x32@2x"), (128, "128x128"), (256, "128x128@2x"),
             (256, "256x256"), (512, "256x256@2x"),
             (512, "512x512"), (1024, "512x512@2x")]
    for px, name in sizes:
        master.resize((px, px), Image.LANCZOS).save(
            os.path.join(iconset_dir, f"icon_{name}.png")
        )
    print(f"icons written under {OUT_DIR}")

if __name__ == "__main__":
    main()
