"""Renders the Auris logo into every icon/image format the build needs.

Run from the repo root after changing the design:  python assets/make_icons.py
Requires Pillow.
"""
from PIL import Image, ImageDraw

S = 1024  # master render size; everything is downsampled from this
BLUE, VIOLET, BG = (138, 180, 255), (196, 161, 255), (28, 29, 33)


def gradient(size):
    """Diagonal blue-to-violet gradient."""
    g = Image.new("RGB", (size, size))
    px = g.load()
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * (size - 1))
            px[x, y] = tuple(round(a + (b - a) * t) for a, b in zip(BLUE, VIOLET))
    return g


def logo(with_background=True):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    if with_background:
        ImageDraw.Draw(img).rounded_rectangle((32, 32, S - 32, S - 32), radius=230, fill=BG + (255,))
    mask = Image.new("L", (S, S), 0)
    d = ImageDraw.Draw(mask)
    cx, cy, r, w = 448, 448, 215, 92  # magnifier ring
    d.ellipse((cx - r, cy - r, cx + r, cy + r), outline=255, width=w)
    d.line((612, 612, 790, 790), fill=255, width=118)  # handle
    d.ellipse((790 - 59, 790 - 59, 790 + 59, 790 + 59), fill=255)  # rounded handle end
    img.paste(gradient(S), (0, 0), mask)
    return img


def main():
    icon = logo()
    sizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
    icon.save("assets/auris.ico", sizes=[(s, s) for s in sizes])
    icon.resize((256, 256), Image.LANCZOS).save("ui/icon.png")

    # Inno Setup wizard images (100% and 200% DPI). BMP without alpha, so composite onto a background.
    for scale in (1, 2):
        small = Image.new("RGB", (55 * scale, 55 * scale), (255, 255, 255))
        mark = icon.resize((55 * scale, 55 * scale), Image.LANCZOS)
        small.paste(mark, (0, 0), mark)
        small.save(f"installer/wizard-small-{scale}x.bmp")

        w, h = 164 * scale, 314 * scale
        large = Image.new("RGB", (w, h), BG)
        glow = gradient(w).resize((w, h)).point(lambda v: v // 6)
        large = Image.blend(large, glow, 0.5)
        mark = logo(with_background=False).resize((110 * scale, 110 * scale), Image.LANCZOS)
        large.paste(mark, ((w - mark.width) // 2, (h - mark.height) // 2), mark)
        large.save(f"installer/wizard-large-{scale}x.bmp")


if __name__ == "__main__":
    main()
