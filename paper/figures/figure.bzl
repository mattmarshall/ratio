"""Typed paper-figure rules (paper-local copy).

Mirrors the convention used by the agora papers, pending the documented
lift of these helpers into `rules_tectonic`. Until then the paper keeps
its own copy, matching the `paper/figures/` layout.

* `tex_figure(name, tex, data)` — one figure/table: a TikZ / pgfplots /
  booktabs template plus zero or more `.dat` files it `\\addplot table`s.
* `paper_figures(name, figures)` — bundle figures (and any other figure
  artifacts, e.g. a rules_puml-rendered `.pdf`) into one filegroup for a
  `tex_section`'s `extra_srcs`.
"""

TexFigureInfo = provider(
    doc = "A typed paper figure or table: a template plus zero or more .dat data files.",
    fields = {
        "tex": "File — TikZ / pgfplots / booktabs template.",
        "data": "list[File] — .dat files referenced via `\\addplot table {...}`.",
        "name": "str — figure name.",
    },
)

def _tex_figure_impl(ctx):
    all_files = [ctx.file.tex] + ctx.files.data
    return [
        DefaultInfo(files = depset(all_files)),
        TexFigureInfo(
            tex = ctx.file.tex,
            data = ctx.files.data,
            name = ctx.label.name,
        ),
    ]

tex_figure = rule(
    implementation = _tex_figure_impl,
    attrs = {
        "tex": attr.label(
            allow_single_file = [".tex"],
            mandatory = True,
            doc = "The TikZ / pgfplots / booktabs template .tex file.",
        ),
        "data": attr.label_list(
            allow_files = [".dat"],
            default = [],
            doc = "Tab-separated data files the template consumes via `\\addplot table {...}`.",
        ),
    },
)

def paper_figures(name, figures, **kwargs):
    """Bundle figure targets into one filegroup for a section's `extra_srcs`.

    `figures` may mix `tex_figure` targets with any other label whose
    `DefaultInfo` carries figure inputs — e.g. a `puml_diagram` target
    that renders a `.pdf` the architecture figure includegraphics-es.
    """
    native.filegroup(
        name = name,
        srcs = figures,
        **kwargs
    )
