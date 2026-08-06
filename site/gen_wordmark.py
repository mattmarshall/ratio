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
FACE = HERE / "fonts" / "ibm-plex-serif-700-latin.woff2"
OUT = HERE / "marks" / "wordmark-ratio.svg"

WORD = "ratio"

# Optical tracking, in 1/1000 em, applied between glyph pairs. The default face
# metrics set "ratio" a shade tight at display size; a touch of air makes the
# lowercase read as a considered wordmark rather than as running text. The `ti`
# pair gets less because the `t`'s crossbar already carries the gap.
TRACKING = 22
PAIR_OVERRIDE = {("t", "i"): 10, ("r", "a"): 16}

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
