#!/usr/bin/env python3
"""Emit `marks/wordmark-ratio.svg` — the word "ratio" as outline paths.

Run this ONCE, by hand, and commit the SVG. It is not part of `build.py`:
the wordmark is a brand asset, not a build product, and the page must build
with nothing but the standard library.

    python3 -m venv .venv && ./.venv/bin/pip install fonttools brotli
    ./.venv/bin/python gen_wordmark.py

Why outlines instead of live text set in the embedded face:

  * the hero never reflows — there is no window in which the wordmark is
    set in a fallback serif and then jumps
  * one asset serves the page, the favicon, the OG image and the README
  * only five glyphs are needed, so the wordmark costs ~2 KB rather than
    the 15 KB of the face it came from

`fill="currentColor"` is deliberate: the mark inherits ink on cream in light
mode and cream on ink in dark, with no second copy of the asset.

⚠️ brotli is required. fontTools reads woff2 through it, and without it the
    import succeeds but `TTFont(...)` fails on the compressed table data.
"""

import pathlib
import sys

try:
    from fontTools.ttLib import TTFont
    from fontTools.pens.svgPathPen import SVGPathPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.pens.boundsPen import BoundsPen
    from fontTools.misc.transform import Transform
except ImportError:  # pragma: no cover - operator error, not a code path
    sys.exit("fontTools missing. See the module docstring for the venv recipe.")

HERE = pathlib.Path(__file__).parent
# IBM Plex MONO, not Serif. The mark is two ledger rules of equal length, and a
# monospaced face is the typographic version of the same idea: fixed advances,
# tabular figures, columns that line up because the metrics make them. The
# serif read as a law firm; this reads as a ledger. It is also the same
# superfamily as the Plex Serif already used for site headings, so the two
# pair rather than collide.
#
# A .ttf rather than a .woff2 on purpose: fontTools needs brotli to read woff2,
# and this face exists only to be cut from — it is never served to a browser,
# so there is nothing to gain from the compressed form and a dependency to lose.
FACE = HERE / "fonts" / "ibm-plex-mono-600-latin.ttf"
OUT = HERE / "marks" / "wordmark-ratio.svg"

WORD = "ratio"

# Optical tracking, in 1/1000 em, applied between glyph pairs.
#
# NEGATIVE here, where the serif wanted positive. A monospaced face gives every
# glyph the same advance, which is right for a column of figures and too airy
# for a five-letter wordmark — the narrow `i` and `t` float in slots sized for
# the `o`. Pulling the whole word in restores the density without touching the
# letterforms, and the `i` gets pulled hardest because its slot is emptiest.
TRACKING = -70
PAIR_OVERRIDE = {("t", "i"): -95, ("i", "o"): -85}

PAD = 40  # viewBox padding in font units, so strokes never clip


def main() -> None:
    if not FACE.is_file():
        sys.exit(f"missing face: {FACE}")

    font = TTFont(FACE)
    upem = font["head"].unitsPerEm
    glyphs = font.getGlyphSet()
    cmap = font.getBestCmap()

    track = TRACKING * upem / 1000.0

    paths: list[str] = []
    bounds = BoundsPen(glyphs)
    x = 0.0
    for i, ch in enumerate(WORD):
        if ord(ch) not in cmap:  # getBestCmap() is keyed by codepoint, not char
            sys.exit(f"face has no glyph for {ch!r}")
        name = cmap[ord(ch)]

        # Flip the y axis: font space is y-up, SVG is y-down.
        xform = Transform(1, 0, 0, -1, x, 0)

        pen = SVGPathPen(glyphs, ntos=lambda v: f"{v:.1f}")
        glyphs[name].draw(TransformPen(pen, xform))
        d = pen.getCommands()
        if d:
            paths.append(d)

        # Accumulate the true ink extent under the same transform.
        glyphs[name].draw(TransformPen(bounds, xform))

        x += glyphs[name].width
        if i < len(WORD) - 1:
            x += PAIR_OVERRIDE.get((ch, WORD[i + 1]), track)

    if bounds.bounds is None:
        sys.exit("no ink drawn — the face produced empty outlines")

    # Crop to the ink, not to the face's ascender/descender box. "ratio" has no
    # descender and only the `t` and the `i` dot reach above the x-height, so the
    # metric box is nearly twice the height of the actual mark — which would make
    # every CSS `height` on the wordmark lie about its optical size.
    ink_x0, ink_y0, ink_x1, ink_y1 = bounds.bounds
    vb_x = ink_x0 - PAD
    vb_y = ink_y0 - PAD
    vb_w = (ink_x1 - ink_x0) + 2 * PAD
    vb_h = (ink_y1 - ink_y0) + 2 * PAD

    body = "\n  ".join(f'<path d="{d}"/>' for d in paths)
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'viewBox="{vb_x:.0f} {vb_y:.0f} {vb_w:.0f} {vb_h:.0f}" '
        f'fill="currentColor" role="img" aria-label="ratio">\n'
        f"  {body}\n"
        f"</svg>\n"
    )

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(svg)
    print(f"{OUT}  {OUT.stat().st_size / 1024:.1f} KB  ({len(paths)} glyph paths, {upem} upem)")


if __name__ == "__main__":
    main()
