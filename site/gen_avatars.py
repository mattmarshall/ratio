#!/usr/bin/env python3
"""Emit an engraved guilloché avatar per person into `marks/avatar-<slug>.svg`.

Run by hand, like gen_wordmark.py, and commit the output:

    python3 -m venv .venv && ./.venv/bin/pip install fonttools brotli
    ./.venv/bin/python gen_avatars.py

WHY THIS EXISTS. The team page needs a portrait slot filled, and the two
obvious answers are both wrong. A LinkedIn photo cannot be republished on a
commercial site without the subject's permission, and a traced or
"pen-and-ink" rendering of one is a derivative of that photograph and still
that person's face. A generic grey silhouette, meanwhile, reads as a broken
image.

So: guilloché. The interlocking line-work engraved on banknotes, share
certificates and bond coupons — which is to say, the exact visual tradition
this product comes from. Each rosette is a hypotrochoid whose parameters are
derived from a hash of the person's name, so it is unique to them, stable
across builds, and depicts nobody. The initials are cut from the same serif as
the wordmark, as outlines, so they need no webfont.

A real photo, when someone supplies one, still wins: build.py prefers
photos/<slug>.* and falls back to this.
"""

import hashlib
import pathlib
import sys
from math import cos, sin, pi

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
OUT = HERE / "marks"

# slug -> initials. Kept here rather than parsed out of team.src.html so that
# adding a person is one edit in one place and the page stays declarative.
PEOPLE = {
    "corrine-duhamel": "CD",
    "saro-getzoyan": "SG",
    "ted-leone": "TL",
    "matt-marshall": "MM",
    "daniel-mcguire": "DM",
    "luis-villa-briongos": "LV",
}

SIZE = 120           # viewBox is SIZE x SIZE
C = SIZE / 2
R_OUTER = 56         # engraved band sits between these two radii
R_INNER = 37         # the monogram has to carry a 56px avatar, so it is
                     # given more of the disc than an engraving would
STEPS = 620          # points per trace; the card renders at 56px, so more than
                     # this is bytes nobody can see. Below ~450 it visibly facets.


def params(slug: str) -> tuple[int, int, float, int]:
    """Deterministic rosette parameters from the name.

    Ranges are chosen so every combination produces a closed, well-formed
    rosette: R/r must not reduce to a small integer or the figure degenerates
    into a few petals, so r is drawn from primes that do not divide R.
    """
    h = hashlib.sha256(slug.encode()).digest()
    R = 60 + h[0] % 24                     # 60..83
    # r = 7 gives only seven coarse petals, which reads as a different kind of
    # object next to the others; the family has to look like one set.
    r = [11, 13, 17, 19, 23, 29][h[1] % 6]  # coprime to R for all R in range
    d = 14 + (h[2] % 18)                   # pen offset -> petal depth
    rings = 2 + h[3] % 2                   # 2 or 3 nested traces
    return R, r, float(d), rings


def hypotrochoid(R: int, r: int, d: float, scale: float) -> str:
    """One closed guilloché trace, as an SVG path."""
    pts = []
    # The curve closes after r/gcd(R,r) revolutions; r is coprime to R here,
    # so r revolutions always closes it exactly.
    turns = r
    for i in range(STEPS + 1):
        t = 2 * pi * turns * i / STEPS
        k = (R - r) / r
        x = (R - r) * cos(t) + d * cos(k * t)
        y = (R - r) * sin(t) - d * sin(k * t)
        pts.append((C + x * scale, C + y * scale))
    head = f"M{pts[0][0]:.1f} {pts[0][1]:.1f}"
    return head + "".join(f"L{x:.1f} {y:.1f}" for x, y in pts[1:]) + "Z"


def initials_path(font: TTFont, text: str) -> str:
    """The initials as outline paths, centered and scaled to the inner disc."""
    glyphs = font.getGlyphSet()
    cmap = font.getBestCmap()
    upem = font["head"].unitsPerEm

    cmds, bounds, x = [], BoundsPen(glyphs), 0.0
    for i, ch in enumerate(text):
        if ord(ch) not in cmap:
            sys.exit(f"face has no glyph for {ch!r}")
        name = cmap[ord(ch)]
        xf = Transform(1, 0, 0, -1, x, 0)          # font space is y-up
        pen = SVGPathPen(glyphs, ntos=lambda v: f"{v:.0f}")
        glyphs[name].draw(TransformPen(pen, xf))
        if pen.getCommands():
            cmds.append(pen.getCommands())
        glyphs[name].draw(TransformPen(bounds, xf))
        x += glyphs[name].width
        if i < len(text) - 1:
            x += 0.03 * upem                        # a little tracking

    if bounds.bounds is None:
        sys.exit("no ink drawn for initials")
    x0, y0, x1, y1 = bounds.bounds
    w, h = x1 - x0, y1 - y0
    scale = (R_INNER * 1.18) / max(w, h * 1.85)     # cap on width, and on height
    tx = C - (x0 + w / 2) * scale
    ty = C - (y0 + h / 2) * scale
    d = " ".join(cmds)
    return f'<g transform="translate({tx:.2f} {ty:.2f}) scale({scale:.4f})"><path d="{d}"/></g>'


def build(slug: str, initials: str, font: TTFont) -> str:
    R, r, d, rings = params(slug)
    scale = R_OUTER / (R - r + d)

    traces = []
    for n in range(rings):
        # Each nested trace is slightly tighter and slightly fainter, which is
        # what gives engraved work its depth.
        s = scale * (1 - 0.085 * n)
        op = 0.55 - 0.13 * n
        traces.append(
            f'<path d="{hypotrochoid(R, r, d + n * 1.6, s)}" fill="none" '
            f'stroke="currentColor" stroke-width="{0.42 - 0.05 * n:.2f}" '
            f'opacity="{op:.2f}"/>'
        )

    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" '
        f'fill="currentColor" role="img" aria-label="{initials}">\n'
        f'  <circle cx="{C}" cy="{C}" r="{R_OUTER + 2}" fill="none" '
        f'stroke="currentColor" stroke-width="0.9" opacity=".45"/>\n'
        f'  <circle cx="{C}" cy="{C}" r="{R_OUTER - 1}" fill="none" '
        f'stroke="currentColor" stroke-width="0.4" opacity=".3"/>\n'
        + "".join(f"  {t}\n" for t in traces)
        + f'  <circle cx="{C}" cy="{C}" r="{R_INNER}" fill="var(--raised, #fff)"/>\n'
        f'  <circle cx="{C}" cy="{C}" r="{R_INNER}" fill="none" '
        f'stroke="currentColor" stroke-width="0.7" opacity=".5"/>\n'
        f"  {initials_path(font, initials)}\n"
        f"</svg>\n"
    )


def main() -> None:
    if not FACE.is_file():
        sys.exit(f"missing face: {FACE}")
    font = TTFont(FACE)
    OUT.mkdir(parents=True, exist_ok=True)
    for slug, initials in PEOPLE.items():
        path = OUT / f"avatar-{slug}.svg"
        path.write_text(build(slug, initials, font))
        R, r, d, rings = params(slug)
        print(f"  {path.name:<34} R={R} r={r} d={d:.0f} rings={rings}  "
              f"{path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    main()
