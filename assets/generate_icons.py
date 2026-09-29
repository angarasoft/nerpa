#!/usr/bin/env python3
"""Generate the nerpa icon set and logo from ``assets/icon.png``.

The mark is a light (silver/blue) emblem drawn on a transparent canvas, so it
needs a dark background to read.  The brand board uses a dark navy (#204070);
we reproduce that for every opaque format and for the logo badge.

Outputs are written into ``assets/``, and the web set (everything a site is
expected to serve) is copied into ``docs/book/_static/`` so Sphinx picks it up:

    logo.svg, logo.png                 horizontal lockup: badge + "nerpa"
    favicon.ico                        16/32/48
    favicon-16x16.png, favicon-32x32.png
    apple-touch-icon.png               180x180
    android-chrome-192x192.png, android-chrome-512x512.png
    og-image.png                       1200x630 social preview
    site.webmanifest

Re-run it after swapping ``assets/icon.png`` for a new mark; everything else is
derived from that one file.
"""

from __future__ import annotations

import base64
import io
import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.dirname(os.path.abspath(__file__))
STATIC = os.path.normpath(os.path.join(ROOT, "..", "docs", "book", "_static"))

NAVY = (32, 64, 112)       # #204070 — brand background
ACCENT = (128, 192, 240)   # #80C0F0 — brand accent
WHITE = (255, 255, 255)

FONT_BOLD = "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf"
FONT_REGULAR = "/usr/share/fonts/dejavu/DejaVuSans.ttf"

RESAMPLE = Image.Resampling.LANCZOS

WORD = "nerpa"
TAGLINE = "infrastructure & configuration"


def load_mark() -> Image.Image:
    """Crop the mark to its opaque content and pad it back to a square."""
    mark = Image.open(os.path.join(ROOT, "icon.png")).convert("RGBA")
    bbox = mark.getchannel("A").getbbox()
    if bbox:
        mark = mark.crop(bbox)
    side = max(mark.size)
    pad = int(round(side * 0.12))
    canvas = Image.new("RGBA", (side + 2 * pad, side + 2 * pad), (0, 0, 0, 0))
    canvas.paste(mark, (pad + (side - mark.size[0]) // 2, pad + (side - mark.size[1]) // 2), mark)
    return canvas


def badge(size: int, radius: int | None = None) -> Image.Image:
    """A navy rounded square (transparent outside the badge)."""
    if radius is None:
        radius = int(round(size * 0.22))
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    ImageDraw.Draw(img).rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=NAVY + (255,))
    return img


def with_mark(canvas: Image.Image, mark: Image.Image, inset: float) -> Image.Image:
    """Paste the square mark centred at ``inset`` of the canvas width."""
    side = int(canvas.size[0] * inset)
    m = mark.resize((side, side), RESAMPLE)
    off = (canvas.size[0] - side) // 2
    canvas.paste(m, (off, off), m)
    return canvas


def font(path: str, size: int) -> ImageFont.FreeTypeFont:
    try:
        return ImageFont.truetype(path, size)
    except OSError:
        return ImageFont.truetype(FONT_BOLD, size)


def text_length(text: str, f: ImageFont.FreeTypeFont) -> int:
    return int(ImageDraw.Draw(Image.new("RGBA", (1, 1))).textlength(text, font=f))


def make_icon(size: int, mark: Image.Image) -> Image.Image:
    return with_mark(badge(size), mark, 0.82)


def build_logo_png(mark: Image.Image) -> Image.Image:
    size = 512
    gap = 56
    f = font(FONT_BOLD, 240)
    tw = text_length(WORD, f)
    logo = Image.new("RGBA", (size + gap + tw, size), (0, 0, 0, 0))
    b = with_mark(badge(size), mark, 0.84)
    logo.paste(b, (0, 0), b)
    ImageDraw.Draw(logo).text((size + gap, size // 2), WORD, font=f, fill=NAVY + (255,), anchor="lm")
    return logo


def build_logo_svg(mark: Image.Image) -> str:
    size = 512
    gap = 56
    inner = int(size * 0.84)
    off = (size - inner) // 2
    f = font(FONT_BOLD, 240)
    tw = text_length(WORD, f)
    width = size + gap + tw

    # Embed the mark at 2x so the SVG stays crisp when scaled up in a browser.
    m = mark.resize((inner * 2, inner * 2), RESAMPLE)
    buf = io.BytesIO()
    m.save(buf, format="PNG")
    b64 = base64.b64encode(buf.getvalue()).decode("ascii")

    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{size}" viewBox="0 0 {width} {size}" role="img" aria-label="{WORD}">
  <rect width="{size}" height="{size}" rx="112" fill="#204070"/>
  <image x="{off}" y="{off}" width="{inner}" height="{inner}" preserveAspectRatio="xMidYMid meet" href="data:image/png;base64,{b64}"/>
  <text x="{size + gap}" y="{size // 2}" dominant-baseline="central" font-family="DejaVu Sans, system-ui, -apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif" font-weight="700" font-size="240" fill="#204070">{WORD}</text>
</svg>
"""

def build_og_image(mark: Image.Image) -> Image.Image:
    w, h = 1200, 630
    img = Image.new("RGBA", (w, h), NAVY + (255,))
    mside = 400
    m = mark.resize((mside, mside), RESAMPLE)
    img.paste(m, (100, (h - mside) // 2), m)

    draw = ImageDraw.Draw(img)
    big = font(FONT_BOLD, 190)
    draw.text((560, 200), WORD, font=big, fill=WHITE + (255,), anchor="lm")

    max_w = w - 560 - 40
    tag = font(FONT_REGULAR, 44)
    if text_length(TAGLINE, tag) > max_w:
        scale = max_w / text_length(TAGLINE, tag)
        tag = font(FONT_REGULAR, max(12, int(44 * scale)))
    draw.text((562, 402), TAGLINE, font=tag, fill=ACCENT + (255,), anchor="lm")
    return img


def main() -> int:
    mark = load_mark()
    os.makedirs(STATIC, exist_ok=True)

    sizes = {16: None, 32: None, 48: None, 180: None, 192: None, 512: None}
    icons = {s: make_icon(s, mark) for s in sizes}

    icons[16].save(os.path.join(ROOT, "favicon-16x16.png"))
    icons[32].save(os.path.join(ROOT, "favicon-32x32.png"))
    icons[48].save(os.path.join(ROOT, "favicon.ico"), format="ICO", sizes=[(16, 16), (32, 32), (48, 48)])

    for name, size in (("apple-touch-icon", 180), ("android-chrome-192x192", 192), ("android-chrome-512x512", 512)):
        icons[size].save(os.path.join(ROOT, f"{name}.png"))

    build_logo_png(mark).save(os.path.join(ROOT, "logo.png"))
    with open(os.path.join(ROOT, "logo.svg"), "w", encoding="utf-8") as fh:
        fh.write(build_logo_svg(mark))

    build_og_image(mark).convert("RGB").save(os.path.join(ROOT, "og-image.png"))

    import json
    manifest = {
        "name": "nerpa",
        "short_name": "nerpa",
        "icons": [
            {"src": "android-chrome-192x192.png", "sizes": "192x192", "type": "image/png"},
            {"src": "android-chrome-512x512.png", "sizes": "512x512", "type": "image/png"},
        ],
        "theme_color": "#204070",
        "background_color": "#204070",
        "display": "standalone",
    }
    with open(os.path.join(ROOT, "site.webmanifest"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2)
        fh.write("\n")

    web = [
        "logo.png", "logo.svg", "favicon.ico", "favicon-16x16.png", "favicon-32x32.png",
        "apple-touch-icon.png", "android-chrome-192x192.png", "android-chrome-512x512.png",
        "og-image.png", "site.webmanifest",
    ]
    for name in web:
        src = os.path.join(ROOT, name)
        dst = os.path.join(STATIC, name)
        with open(src, "rb") as s, open(dst, "wb") as d:
            d.write(s.read())

    for name in sorted(os.listdir(ROOT)):
        if name in ("icon.png", "brand-sheet.png", "generate_icons.py", "generate-icons.sh"):
            continue
        print(f"  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

