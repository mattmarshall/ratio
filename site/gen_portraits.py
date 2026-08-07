#!/usr/bin/env python3
"""Render a supplied headshot into the house engraved-portrait treatment.

    python3 -m venv .venv && ./.venv/bin/pip install Pillow
    ./.venv/bin/python gen_portraits.py            # process everything in photos/src/
    ./.venv/bin/python gen_portraits.py --demo     # synthetic test image, no faces

Reads  photos/src/<slug>.{jpg,jpeg,png,webp}   the original, as supplied
Writes photos/<slug>.png                        the treated version build.py inlines

⛔ ONLY RUN THIS ON PHOTOGRAPHS THE SUBJECT HAS GIVEN YOU FOR THIS PURPOSE.
Do not point it at a LinkedIn profile picture. A filtered, screened or
line-rendered photograph is still a derivative of that photograph and still
that person's face; running it through this script launders nothing. The
consent you need is to publish their likeness on a commercial page, and no
amount of styling substitutes for it. Where a portrait is missing, the
guilloché monogram from gen_avatars.py stands in perfectly well.

THE TREATMENT. Photographs sit badly on this site: six headshots taken in six
different rooms under six different white balances read as a collage, not a
brand. So each is reduced the way a nineteenth-century engraver would have
reduced it for a share certificate — desaturated, contrast-shaped, and screened
to a duotone in the page's own two inks. The result carries the same weight as
the guilloché avatars beside it, and a missing portrait no longer looks like a
hole.
"""

import argparse
import pathlib
import sys

try:
    from PIL import Image, ImageEnhance, ImageFilter, ImageOps
except ImportError:  # pragma: no cover - operator error, not a code path
    sys.exit("Pillow missing. See the module docstring for the venv recipe.")

HERE = pathlib.Path(__file__).parent
SRC = HERE / "photos" / "src"
OUT = HERE / "photos"

SIZE = 400                       # rendered square; the card shows it at 56px
EXTS = (".jpg", ".jpeg", ".png", ".webp")

# The two inks. Deliberately the light theme's ground and its deepest green:
# the portrait then reads as printed ON the page rather than pasted onto it,
# and it still holds up on the dark ground because the contrast is internal.
INK_DARK = (15, 36, 24)          # --text  #0F2418
INK_LIGHT = (252, 251, 247)      # --ground #FCFBF7


def treat(im: Image.Image) -> Image.Image:
    """Square-crop, desaturate, shape contrast, and duotone."""
    im = ImageOps.exif_transpose(im).convert("RGB")

    # Square crop about the center-top: headshots put the face above center, so
    # a plain center crop tends to take the chin off.
    w, h = im.size
    s = min(w, h)
    top = int((h - s) * 0.32) if h > s else 0
    left = (w - s) // 2
    im = im.crop((left, top, left + s, top + s)).resize((SIZE, SIZE), Image.LANCZOS)

    g = ImageOps.grayscale(im)
    g = ImageOps.autocontrast(g, cutoff=(2, 1))
    # Unsharp before the tonal squeeze: engraving reads by edge, not by tone.
    g = g.filter(ImageFilter.UnsharpMask(radius=2.5, percent=115, threshold=3))
    g = ImageEnhance.Contrast(g).enhance(1.18)
    # Lift the shadows slightly so the darkest ink never blocks up solid.
    g = g.point(lambda v: int(18 + v * 0.87))

    duo = ImageOps.colorize(g, black=INK_DARK, white=INK_LIGHT)

    # Circular alpha, matching the avatar discs it sits beside.
    mask = Image.new("L", (SIZE * 4, SIZE * 4), 0)
    from PIL import ImageDraw
    ImageDraw.Draw(mask).ellipse((0, 0, SIZE * 4 - 1, SIZE * 4 - 1), fill=255)
    mask = mask.resize((SIZE, SIZE), Image.LANCZOS)      # supersampled edge
    out = duo.convert("RGBA")
    out.putalpha(mask)
    return out


def demo() -> Image.Image:
    """A synthetic gradient-and-shapes image, so the treatment can be checked
    without pointing this script at anybody's face."""
    from PIL import ImageDraw
    im = Image.new("RGB", (SIZE, SIZE))
    d = ImageDraw.Draw(im)
    for y in range(SIZE):
        d.line([(0, y), (SIZE, y)], fill=(60 + y // 3, 70 + y // 4, 90 + y // 5))
    d.ellipse((110, 90, 290, 290), fill=(210, 200, 185))
    d.ellipse((150, 150, 175, 175), fill=(40, 40, 45))
    d.ellipse((225, 150, 250, 175), fill=(40, 40, 45))
    d.arc((160, 190, 240, 250), start=15, end=165, fill=(70, 60, 55), width=6)
    return im


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--demo", action="store_true",
                    help="run the treatment on a synthetic image and write photos/_demo.png")
    args = ap.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)

    if args.demo:
        p = OUT / "_demo.png"
        treat(demo()).save(p)
        print(f"  {p}  {p.stat().st_size / 1024:.0f} KB  (synthetic — delete before shipping)")
        return

    if not SRC.is_dir():
        sys.exit(f"no source directory: {SRC}\n"
                 "Create it and drop in headshots the subjects have given you.")

    found = [f for f in sorted(SRC.iterdir()) if f.suffix.lower() in EXTS]
    if not found:
        print(f"nothing to do — no images in {SRC}")
        return
    for f in found:
        out = OUT / f"{f.stem}.png"
        treat(Image.open(f)).save(out, optimize=True)
        print(f"  {f.name:<30} -> {out.name:<30} {out.stat().st_size / 1024:>5.0f} KB")


if __name__ == "__main__":
    main()
