"""Per-component competitive spec PDFs.

Each component is a standalone `<slug>.tex` that `\\input`s the shared
`_preamble.tex` and compiles to its own PDF via rules_tectonic. The
component catalog was scaffolded from a June 2026 scrape of the published
component/feature architecture of SS&C Black Diamond, Orion, Addepar, and
Envestnet Tamarac.

  component_specs(["portfolio-accounting", "trading-rebalancing", ...])

emits one `tectonic_pdf` per slug plus an `all_specs` filegroup that builds
them all (`bazel build //competitive/specs:all_specs`).
"""

load("@rules_tectonic//tectonic:defs.bzl", "tectonic_pdf")

# Shared inputs every component PDF stages: the preamble and the logo.
_COMMON_SRCS = ["_preamble.tex", "//images:ratio.png"]

def component_spec(slug):
    """One competitive component spec → `<slug>.pdf`."""
    tectonic_pdf(
        name = slug,
        main = slug + ".tex",
        srcs = _COMMON_SRCS,
    )

def component_specs(slugs):
    """Declare every component spec + an `all` filegroup over their PDFs."""
    for slug in slugs:
        component_spec(slug)
    native.filegroup(
        name = "all_specs",
        srcs = [":" + slug for slug in slugs],
    )
