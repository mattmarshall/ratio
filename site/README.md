# site — the Ratio marketing pages

Seven self-contained HTML pages built from one design system, for audiences that
share very little vocabulary.

| Source | Output | Written for |
|---|---|---|
| `index.src.html` | `index.html` | Fund accountants, controllers, operations leads. Sells outcomes: the close, breaks, restatements, audit requests, the bill. |
| `workflows.src.html` | `workflows.html` | Whoever would run the implementation. How expertise becomes enforced rules, walked through on cross-border dividend withholding. |
| `compliance.src.html` | `compliance.html` | CFOs, heads of compliance, controls leads. Five questions an examiner actually asks, and how each resolves to a query. |
| `platform.src.html` | `platform.html` | COOs, heads of fund accounting, evaluating CTOs. The two planes, the planned NAV run, what it costs to run, the data estate, the ecosystem. |
| `roadmap.src.html` | `roadmap.html` | Anyone asking "when". The route to parity across the sixteen advertised capability areas. |
| `technical.src.html` | `technical.html` | Engineers and technical due diligence. The Lean theorem and the proof-to-artifact pipeline. |
| `team.src.html` | `team.html` | ⛔ **Deliberately unfinished.** Roles and bios are `TODO`; `verify.py` fails the build while any remain. |

Shared chrome lives in `style.css`; the nav is duplicated per page so each can
carry its own `aria-current`.

## The team page is blocked on purpose

It ships with `TODO` in every role and bio, and `verify.py` fails while they are
there — so the site cannot deploy with them. That is deliberate: these are six
real, named people, and inventing a plausible job title or career for someone
who has not seen it is a fabrication about a person who cannot easily correct
it. LinkedIn returns HTTP 999 to automated fetches and an authwall to a browser
that is not signed in, so nothing was readable anyway.

Names were inferred from the profile URL slugs and need checking — particularly
"Corrine" and whether Luis's surname is hyphenated.

**Photos** go in `photos/<slug>.jpg|png` and are inlined by `build.py`, with an
initials monogram standing in where one is absent. They must be inlined rather
than linked, because an external image URL is blocked by the Artifact CSP and
failed by `verify.py`. Do not take them from LinkedIn — a profile photo belongs
to its subject or photographer; ask each person for one.

**The register split is enforced, not trusted.** `verify.py` fails the build if
the practitioner page acquires words like *kernel*, *conservation*, *Lean* or
*GPU*. If you find yourself wanting one there, the content belongs on
`platform` or `technical` instead.

## Build

```sh
python3 build.py                                      # all seven, as body fragments
python3 build.py --standalone -d _site                # all seven, complete documents
python3 build.py --page platform --standalone -d _site   # just one
python3 build.py --check-fonts                        # verify vendored faces first
python3 verify.py _site                               # the checks CI runs
```

Standard library only. No network, no dependencies, no Bazel.

**Edit the `.src.html` files and `style.css`.** The `.html` outputs are
regenerated on every build and are gitignored — a change you make there will be
clobbered without warning.

### The two output shapes

`*.src.html` are **body fragments**: they open with `<title>` and carry no
`<!doctype>`, `<html>`, `<head>` or `<body>`. That is what the Claude Artifact
host wants, since it supplies those itself.

Anything serving the files *directly* — GitHub Pages, a bucket, `python3 -m
http.server` — must use `--standalone`, which wraps each fragment in a real
document and adds `charset`, `viewport`, `color-scheme` and a per-page
description. Served raw, a fragment renders in **quirks mode** with no viewport
meta and lays out at 980px on a phone.

### Cross-links

The pages link to each other by relative filename, which is correct for a
directory of files and wrong for the Artifact host, where each page gets its own
opaque URL. `--link SLUG=URL` rewrites them at publish time:

```sh
python3 build.py --page index -d out \
  --link platform=https://… --link roadmap=https://… --link technical=https://…
```

## Publishing to GitHub Pages

`.github/workflows/site.yml` builds and verifies on every push and pull request
touching `site/`, and deploys to <https://mattmarshall.github.io/ratio/> on
pushes to `main`. Build and deploy are separate jobs — a PR is checked but
cannot publish.

⚠️ **Pages must still be enabled once, by hand**, in *Settings → Pages →
Source: GitHub Actions*. Until that is done the deploy job fails; the build job
passes regardless, so a red deploy on the first run means exactly this and
nothing worse. Publishing is a decision, so it is deliberately not automated
away — see the checklist at the foot of this file.

## What gets inlined, and why

Each page ends up fully self-contained, because the Artifact CSP blocks every
external subresource — a stylesheet, a font, or an image on another host simply
does not load. `verify.py` asserts this.

- **`style.css`** — the shared design system, inlined into all seven pages. It
  lives in one file so they cannot drift apart. The **fonts are substituted into
  the CSS, not into the pages**, so each face is base64-encoded once per build
  rather than once per page.
- **`fonts/*.woff2`** — as `data:` URIs. `fonts/SOURCES.json` carries the sha256
  and upstream URL of each; `--check-fonts` verifies them. Both families are
  SIL OFL 1.1 and the licenses ship alongside. They are byte-identical copies of
  `/Volumes/Workspace/aion/brand/fonts`, which `build.py` will fall back to.
- **`marks/*.svg`** — the wordmark and the mark. Their design-rationale comments
  are stripped on the way in; that reasoning belongs in the repo, not in every
  byte served to every reader.

## The marks

`marks/wordmark-ratio.svg` is **generated, then committed**. `gen_wordmark.py`
pulls the `r a t i o` outlines out of IBM Plex Serif 700 and writes them as
paths, so the hero never flashes an unstyled fallback and one asset serves the
page, a favicon and the README.

```sh
python3 -m venv .venv && ./.venv/bin/pip install fonttools brotli
./.venv/bin/python gen_wordmark.py
```

⚠️ **`brotli` is required** — fontTools reads woff2 through it, and without it
the import succeeds but `TTFont(...)` fails on the compressed table data. This is
the *only* part of the site that needs a dependency, which is exactly why it is a
separate hand-run script and not part of `build.py`.

`marks/mark-ratio.svg` is drawn by hand: the open ledger of the original logo
reduced to two facing columns of equal ink closed by a rule. An earlier version
drew the entries as stacked rules and turned to mush below 24px; three solid
shapes was what survived a 16px favicon.

`images/ratio.png` — the original logo — is **not used here** (it bakes in the
old "CLI/TUI personal finance application" strapline and is raster only) and is
**not deleted**: `//paper`, `//marketing` and `//competitive/specs` all still
`\includegraphics` it.

## Licensing position

✅ **SETTLED 2026-08-10: AGPL-3.0, with a commercial license alongside it.** See
[../LICENSING.md](../LICENSING.md). Readable so a client, an auditor or a
regulator can check the arithmetic; reciprocal so that a modification made by
someone operating it over a network comes back rather than becoming a private
advantage over the people relying on it to be correct.

⛔ **THE "NOT OPEN SOURCE" LINE IS GONE, AND SO ARE THE CHECKS THAT ENFORCED IT.**
AGPL-3.0 is OSI- and FSF-approved. Banning the phrase would have made
`verify.py` and `//marketing:language_test` assert something false about our own
license, which is worse than not checking: it defends the error. What is
commercially load-bearing is **reciprocity**, not closedness — you may read it,
run it and modify it, and §13 means you may not take it private.

⛔ **AND "self-host" IS NO LONGER BANNED, because the AGPL permits it.** The old
note claimed the license made "operating the stack as a competing service
impractical without a commercial agreement". That was never true of the AGPL: it
requires a competitor to publish their modifications, it does not forbid the
service. A license that forbids it — BUSL-1.1, Elastic v2 — is neither copyleft
nor open source, and would trade the verification argument for the exclusion.
That trade has not been made.

What still must not appear: "MIT licen", "open core", "permissively licensed"
(except as "not permissively licensed"), and "use it however you like".

This matters for the copy because the *verification* argument used to lean on
openness. It still holds, on three legs that survive the license change:
published, checkable proofs; deterministic replay the customer can run
themselves; and data portability in open export formats. **"Read the arithmetic"
stays. "Use it however you like" goes.** MIT, "open source", "open core",
"self-host the open core" and links to a public repository have all been removed
— `verify.py` does not police this, so it is on review.

✅ Confirmed: **AGPL-3.0**. Plain GPL leaves the SaaS loophole open — a modified
version offered as a service triggers no obligation at all — and §13 is what
closes it.

⚠ This paragraph used to end "and would not prevent a competitor operating the
stack", implying AGPL does. It does not, and no copyleft license does. What it
prevents is a competitor operating a modified version *privately*.

## Where the numbers on the platform page come from

Every performance figure is **modeled, never measured**, and the page says so
three times. They derive from the whitepaper's per-record cost —
`paper/figures/data/throughput.dat`, 6 ns per record, flat in rule-set size —
plus ordinary assumptions about record sizes and bandwidth:

| Input | Value |
|---|---|
| Tax lots | 20,000,000 |
| Lot record | 80 B packed |
| Kernel operations per lot | ~40 |
| Postings emitted per lot | 3 @ 48 B |
| Fetch bandwidth | 800 MB/s aggregate, 16 lanes |
| Fold parallelism | 32-way CPU; 16,896 lanes quoted for an H100 |
| On-demand GPU rate | $3.00/hr |

**The published headline is 10–15 minutes, not seconds, and that is deliberate.**
The arithmetic really is milliseconds — but a NAV run has to pull from
custodians, pricing vendors and administrators, and those are other people's
systems. Promising seconds would be promising something we do not control. The
incumbent contrast (~1 hour) is a broad industry observation, not a measurement
of a named product.

The data-estate figures — ~$150/yr all-in at 20M lots — cover storage at rest
(hot + archive tiers), the CPU that waits on ingest, the GPU burst for the fold,
and egress. Quoting compute alone would have made Ratio look like a cheap batch
job sitting on top of an expensive relational estate, which is exactly the cost
structure it exists to avoid: **there is no RDBMS underneath, so there is no
per-core license.**

⛔ **Do not restate any of these as achieved performance.** The workload planner
they describe is designed, not built, and nothing has been run on a
20-million-lot book. The 20M workload is a **scale illustration and not a
customer** — an earlier draft named a specific fund complex, which on a public
page would read as a reference customer. Keep it unnamed.

## Traps

- **Never name a placeholder in its own syntax anywhere in a source file**, not
  even inside an HTML comment. `build.py` replaces *every* occurrence, so a token
  mentioned in documentation gets a whole font inlined into that comment —
  silently, since a comment renders as nothing. This shipped a 226 KB page (vs.
  141 KB) before `substitute()` started insisting on exactly one occurrence.
  Describe placeholders in prose, as the source files now do.
- ⚠️ **The palette has diverged from the LaTeX and needs reconciling.** The site
  is green-on-cream (green-bar accounting paper); `//marketing/main.tex` and
  `//competitive/specs/_preamble.tex` are still the original ledger-sepia, so
  the PDFs and the website currently wear different brands. Deciding which wins
  is an outstanding call, not an oversight.
- **Contrast is computed, not eyeballed.** Every text/background pair in both
  themes clears WCAG AA (4.5:1), and borders clear 3:1. The first green draft
  failed in four places — light-mode amber was 2.55:1 — and the first dark draft
  was a saturated green ground that was legible on paper and tiring in practice.
  Dark is now near-neutral charcoal with green reserved for the accent and the
  status colors. Re-check with the snippet in this repo's history before
  changing any token.
- **The chart palette is validated, not eyeballed.** The brief's own pairing
  (`ratioBrown` against `posGreen`) fails a colorblind-separation check — brown
  reads as gray and the two lines sit below the normal-vision distinguishability
  floor. The series colors here (`#BC4A18`/`#2E7D52` light, `#CE7A33`/`#4CA87A`
  dark) were chosen because they pass in both themes. Re-validate before changing
  them.
- **SVG text does not wrap and will not warn you.** Two labels have already been
  clipped by a viewBox that was too narrow. After editing a figure, check every
  `<text>` against the viewBox — accounting for each element's own `transform`,
  since `getBBox()` ignores it and will report a rotated axis label as
  overflowing when it is fine.
- **The roadmap's sixteen components mirror `//competitive/specs/catalog.tex`**,
  including its tier labels. If that catalog gains or re-places a component, the
  roadmap page is wrong until updated.
- **Load-bearing claims that must not be trimmed**: the competitive matrix's
  fairness-and-trademark note; the "illustrative, not a quote" labels on the cost
  chart and trial balance; the modeled-not-measured caveats on the platform
  page; and the status sections on `index` and `roadmap`.
- **The status sections are the point, not a hedge.** Ratio's product surface is
  far behind its foundation. A site that asks the reader to check rather than
  trust has to be accurate about itself, or it argues against itself.
- **The roadmap's built / not-yet-built columns are checked against PLAN.md.**
  `verify.py` fails if the page still lists tax lots, FX translation, corporate
  actions, journal persistence, wash sales, MinTax ranking, SpecID named
  selection, or average-cost pooling as unbuilt, or if it claims Postgres is
  running. Postgres stays on the spec side: Stage E is open, and PLAN's
  "four of these were built" table records it as spec-only. Wash sales
  are the engine window and the attach write (#133 / #138) plus
  `WashRestatement` as a citeable record and the non-US
  holding-period variant as an election
  (`wash_keep_holding_period`; unset, not a silent keep). The
  console cites those flags on the fund lot-terms screen; unset
  stays unset. MinTax is a ranking at a price
  (`min_tax_short_weight`; unset, not a silent 2), not an Order. SpecID
  is a named selection (`identified_lots`; unnamed or overspecified
  refuse; `lot_method = "specific_id"` stays refused), not an Order,
  not a UI. Average cost is a pool (`average_cost = true`; unset, not
  a silent true), not an Order, and not a console election screen.

## American English

House style, per the repo owner's global preference: **American spellings
everywhere** — "modeled", "color", "license", "organized", "gray", "judgment".
An earlier draft of this site was written in British English throughout and had
to be swept; if you are adding copy, write it American first.

## No Bazel target (yet)

Every other artifact in this repo builds under Bazel; this one does not, on
purpose. `rules_python` is not in the module graph at all, so a `py_binary`
would mean adding a `bazel_dep` — and the alternative, a `genrule` shelling out
to whatever `python3` is on `PATH`, would put a non-hermetic step into a repo
whose whole argument is reproducibility. Both are the repo owner's call rather
than a side effect of adding a website, so for now: run `build.py`.

## Before this goes public

- [ ] **Enable Pages** — *Settings → Pages → Source: GitHub Actions*. Until then
      the deploy job fails by design.
- [ ] `https://github.com/mattmarshall/ratio` is linked from three pages.
      Confirm the repository is public, or change the links — a 404 on "read the
      source" is worse than not offering it.
- [ ] The CTAs ("start a shadow run", "read the…") have no destination for a
      reader who wants to make contact. Decide what happens when someone says yes.
- [ ] Competitive claims about Black Diamond, Orion, Addepar and Tamarac are
      Ratio's reading of public material as of 2026. Worth a second pair of eyes
      before this is a public, indexable page.
- [ ] The platform page's cost model invites arithmetic-checking by exactly the
      kind of reader it is aimed at. Have someone adversarial re-derive it.
