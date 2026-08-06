# Competitive component specs

Per-component spec **scaffolds** for the Ratio ecosystem, reverse-engineered from
the *advertised* component/feature architecture of the leading wealth-tech
platforms. Each component compiles to its own PDF via
[rules_tectonic](https://github.com/fastverk/rules_tectonic).

## Build

```bash
bazel build //competitive/specs:all_specs        # catalog + every component
bazel build //competitive/specs:catalog          # the master overview table
bazel build //competitive/specs:portfolio-accounting   # a single component
```

PDFs land in `bazel-bin/competitive/specs/<slug>.pdf`.

## What this is

Each `<slug>.tex` is a one-page spec scaffold recording, for one component:

- **Advertised by** — which incumbents publish this component and under what name.
- **Advertised capabilities (feature level)** — what they market it does.
- **Ratio mapping** — how the component maps onto Ratio's proven-kernel
  architecture (trusted core / control plane / fact plane / API / extensions /
  fenced AI authoring).
- **Conservation & trust placement** — where it sits relative to the
  machine-checked kernel.
- **References** — the source URLs.

`catalog.tex` is the master overview: the full component matrix (coverage per
vendor) plus each component's Ratio placement.

## Sources (scraped June 2026)

- **SS&C Black Diamond** (primary anchor) — <https://www.sscblackdiamond.com/>,
  `/solutions/investment-management/`, `/solutions/integrations/`
- **Orion Advisor Tech** — <https://orion.com/advisor-tech> (+ `/portfolio-accounting`, `/trading`)
- **Addepar** — <https://addepar.com/wealth-management>, `/why-addepar`
- **Envestnet Tamarac** — <https://www.envestnet.com/tamarac>, `/wealth-management/crm`

## Status & disclaimer

These are **scaffolds**, not finished specs — expand each with a data model, API
surface, and acceptance criteria when prioritized. Capability and coverage marks
reflect Ratio's reading of public marketing pages as of June 2026, not a feature
audit, and vendor capabilities change. Black Diamond is a trademark of SS&C
Technologies; Orion, Addepar, and Tamarac are trademarks of their respective
owners. Ratio is an independent project and is not affiliated with any of them.
