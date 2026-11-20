---
name: restricted-network-bazel
description: Make Bazel work in a sandbox that refuses GitHub archive downloads (HTTP 403 on github.com/<owner>/<repo>/archive/...) but allows git clones — the usual state of a cloud agent container. Use when any Bazel command fails in analysis with "GET returned 403 Forbidden" or "no such package '@@rules_...+//...'" before compiling anything.
---

# Bazel behind a network that blocks GitHub archives

## Symptom

```
WARNING: Download from https://github.com/tomato-bazel/rules_tectonic/archive/refs/tags/v0.2.0.tar.gz failed: ... GET returned 403 Forbidden
ERROR: ... no such package '@@rules_tectonic+//tectonic'
```

Every Bazel module whose registry source is a GitHub `/archive/` URL is
affected. GitHub *release* downloads (the Lean toolchain, `tla2tools.jar`) and
`git clone` usually still work. ⛔ A 403 from a proxy is a policy decision: do
not retry it or tunnel around it. Use the permitted channel — git.

## Fix

```bash
python3 .agents/skills/restricted-network-bazel/scripts/override_github_archives.py
```

It runs `bazel fetch --keep_going //...`, reads which GitHub archives were
refused, and for each one a registry module in the committed
`MODULE.bazel.lock` points at:

1. shallow-fetches the pinned tag or commit with git and exports the tree,
2. applies the registry's patches and overlay files, each verified against the
   integrity hash the registry publishes,
3. writes `common --override_module=<name>=<dir>` into a marked block in
   `~/.bazelrc`.

It repeats until the fetch is clean, because one failure hides the next. On
this repository it took three rounds and seven modules: `rules_aip`,
`rules_lean`, `rules_puml`, `rules_tectonic`, `rules_tla`, then `rules_github`,
then `googleapis`. On an unrestricted network the first fetch is clean and it
writes nothing.

Options: `--targets '//crates/...'` to fetch less, `--dest DIR` for the
checkouts (default `~/.cache/ratio-bazel-overrides`), `--bazelrc FILE`, and
`--remove` to delete the block.

## Rules

- ⛔ **Never commit what this changes.** With overrides in force, Bazel
  rewrites `MODULE.bazel.lock` (overridden modules drop out). Before every
  commit: `git checkout -- MODULE.bazel.lock`. The overrides live in your user
  bazelrc, never the repository's `.bazelrc`.
- ⛔ **Override only what a fetch reports.** Overriding every blocked module
  looks safer and breaks the build: `rules_android` 0.1.1 is never fetched
  here, and making it present turns on a Bazel autoload that fails on its
  missing `rules/BUILD`. The script asks Bazel for exactly this reason.
- ⚠ Patches are applied before overlays, so an overlay wins where both write
  one file. For `googleapis`, the only module here with both, they produce the
  same `MODULE.bazel`.
- ⚠ A blocked archive that is not a registry module (a plain `http_archive`
  in a module extension) is reported as still blocked. Handle it by hand, or
  ask for the host to be allowed.
- After it finishes, `bazel test //marketing:language_test` is a quick proof
  that the tectonic rules load.
