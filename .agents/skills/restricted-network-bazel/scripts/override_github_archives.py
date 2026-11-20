#!/usr/bin/env python3
"""Supply blocked GitHub archive downloads to Bazel from git clones instead.

Some sandboxes (cloud agent containers among them) allow `git clone` of public
GitHub repositories but refuse `github.com/<owner>/<repo>/archive/...`
downloads with HTTP 403. Bazel fetches most registry modules that way, so a
build dies in analysis with `GET returned 403 Forbidden` before compiling
anything.

This runs `bazel fetch --keep_going` over the targets you name and reads which
GitHub archives Bazel failed to download. For each one that a registry module
in the committed MODULE.bazel.lock points at, it:

  1. shallow-fetches the pinned tag or commit with git,
  2. exports the tree (no .git) into --dest/<module>-<version>,
  3. applies the registry's patches, then its overlay files, each checked
     against the integrity hash the registry publishes,
  4. writes `common --override_module=<module>=<dir>` into a marked block of
     --bazelrc (default ~/.bazelrc), replacing any earlier block.

It repeats until a fetch reports no blocked archive it can fix: one failure can
hide the next (googleapis only surfaced after the tomato-bazel rules loaded).
On an unrestricted network the first fetch is clean and nothing is written.

⛔ OVERRIDE ONLY WHAT A FETCH REPORTS. Overriding every blocked module looks
safer and is not: rules_android 0.1.1 is never fetched by this build, and
making it present turns on a Bazel autoload that fails on its missing
`rules/BUILD`. That is why this asks Bazel instead of guessing.

⛔ LOCAL ONLY. The overrides make Bazel rewrite MODULE.bazel.lock (overridden
modules drop out of it). Never commit that: `git checkout -- MODULE.bazel.lock`
before you commit, and never put the overrides in the repository's .bazelrc.

⚠ Patches are applied before overlay files, so an overlay wins where both
write one file. For googleapis, the one module here with both, the two produce
the same MODULE.bazel.

Usage:
  override_github_archives.py [--targets '//...'] [--dest DIR] [--bazelrc FILE]
  override_github_archives.py --remove [--bazelrc FILE]
"""

import argparse
import base64
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

BEGIN = "# >>> ratio: github archive overrides (local only, never commit) >>>"
END = "# <<< ratio: github archive overrides <<<"
ARCHIVE = re.compile(
    r"^https://github\.com/(?P<owner>[^/]+)/(?P<repo>[^/]+)/archive/"
    r"(?P<ref>.+?)\.(?:tar\.gz|tgz|zip)$"
)
# Bazel names a refused download in either of these shapes.
FAILED = re.compile(
    r"(?:Error downloading \[|Download from )(https://github\.com/[^\] ]+/archive/[^\] ]+)"
)
MAX_ROUNDS = 8


def curl(url: str) -> bytes:
    return subprocess.run(
        ["curl", "-sSfL", "--max-time", "60", url], check=True, capture_output=True
    ).stdout


def check_integrity(data: bytes, integrity: str, what: str) -> None:
    algo, _, want = integrity.partition("-")
    if algo != "sha256":
        raise RuntimeError(f"{what}: unsupported integrity {algo}")
    got = base64.b64encode(hashlib.sha256(data).digest()).decode()
    if got != want:
        raise RuntimeError(f"{what}: integrity mismatch (got sha256-{got}, want {integrity})")


def export_tree(owner: str, repo: str, ref: str, into: pathlib.Path) -> None:
    """Shallow-fetch one ref and export its tree, without .git, into `into`."""
    with tempfile.TemporaryDirectory() as tmp:
        g = ["git", "-C", tmp]
        subprocess.run(["git", "init", "-q", tmp], check=True)
        subprocess.run(g + ["remote", "add", "origin", f"https://github.com/{owner}/{repo}"],
                       check=True)
        # A tag, a branch or a commit: `fetch <ref>` takes all three.
        subprocess.run(g + ["fetch", "-q", "--depth", "1", "origin", ref],
                       check=True, env={**os.environ, "GIT_LFS_SKIP_SMUDGE": "1"})
        into.mkdir(parents=True)
        tar = subprocess.run(g + ["archive", "--format=tar", "FETCH_HEAD"],
                             check=True, capture_output=True).stdout
        subprocess.run(["tar", "-x", "-C", str(into)], input=tar, check=True)


def prepare(mod: dict, dest: pathlib.Path) -> pathlib.Path:
    """Build a local module directory equivalent to the registry's archive."""
    name, version, source = mod["name"], mod["version"], mod["source"]
    m = ARCHIVE.match(mod["url"])
    ref = m["ref"].removeprefix("refs/tags/").removeprefix("refs/heads/")
    print(f"  {name} {version}: git {m['owner']}/{m['repo']} @ {ref}")
    root = dest / f"{name}-{version}"
    if root.exists():
        shutil.rmtree(root)
    export_tree(m["owner"], m["repo"], ref, root)

    # strip_prefix is "<archive top dir>[/<subdir>]"; a git export has no top
    # dir, so only the remainder applies.
    module_dir = root
    sp = source.get("strip_prefix", "")
    if "/" in sp:
        module_dir = root / sp.split("/", 1)[1]

    base = f"{mod['registry']}/modules/{name}/{version}"
    strip = int(source.get("patch_strip", 0))
    for patch, integrity in (source.get("patches") or {}).items():
        data = curl(f"{base}/patches/{patch}")
        check_integrity(data, integrity, f"{name} patch {patch}")
        r = subprocess.run(["patch", "-s", "-f", f"-p{strip}", "-d", str(module_dir)],
                           input=data, capture_output=True)
        if r.returncode != 0:
            raise RuntimeError(f"{name}: patch {patch} did not apply:\n"
                               f"{r.stdout.decode()}{r.stderr.decode()}")
        print(f"    patched {patch}")
    for rel, integrity in (source.get("overlay") or {}).items():
        data = curl(f"{base}/overlay/{rel}")
        check_integrity(data, integrity, f"{name} overlay {rel}")
        target = module_dir / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        print(f"    overlay {rel}")
    return module_dir


def write_block(bazelrc: pathlib.Path, lines: list[str]) -> None:
    text = bazelrc.read_text() if bazelrc.exists() else ""
    text = re.sub(rf"\n?{re.escape(BEGIN)}.*?{re.escape(END)}\n?", "\n", text, flags=re.S)
    text = text.strip("\n")
    if lines:
        text += ("\n\n" if text else "") + "\n".join([BEGIN, *lines, END])
    bazelrc.write_text(text + "\n" if text else "")


def registry_archives(top: str) -> dict[str, dict]:
    """Every registry module source in the COMMITTED lockfile, keyed by archive URL.

    ⚠ The committed copy, not the working one: overrides already in force drop
    their modules from the working lockfile, and those are exactly the ones a
    rerun has to re-create.
    """
    lock = json.loads(subprocess.run(["git", "-C", top, "show", "HEAD:MODULE.bazel.lock"],
                                     check=True, capture_output=True).stdout)
    out = {}
    for key, digest in lock.get("registryFileHashes", {}).items():
        m = re.match(r"^(?P<reg>https://.+)/modules/(?P<name>[^/]+)/(?P<ver>[^/]+)/source\.json$", key)
        if not m or digest == "not found":
            continue
        raw = curl(key)
        if hashlib.sha256(raw).hexdigest() != digest:
            raise RuntimeError(f"{key} does not match the lockfile's hash")
        source = json.loads(raw)
        url = source.get("url") or (source.get("urls") or [""])[0]
        if ARCHIVE.match(url):
            out[url] = {"name": m["name"], "version": m["ver"], "registry": m["reg"],
                        "source": source, "url": url}
    return out


def blocked_downloads(top: str, targets: list[str]) -> set[str]:
    r = subprocess.run(["bazel", "fetch", "--keep_going", "--", *targets],
                       cwd=top, capture_output=True, text=True)
    return set(FAILED.findall(r.stdout + r.stderr))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--targets", default="//...",
                    help="space-separated target patterns to fetch (default //...)")
    ap.add_argument("--dest", default=str(pathlib.Path.home() / ".cache/ratio-bazel-overrides"))
    ap.add_argument("--bazelrc", default=str(pathlib.Path.home() / ".bazelrc"))
    ap.add_argument("--remove", action="store_true", help="delete the override block and exit")
    args = ap.parse_args()

    bazelrc = pathlib.Path(args.bazelrc).expanduser()
    if args.remove:
        write_block(bazelrc, [])
        print(f"removed the override block from {bazelrc}")
        return 0

    top = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                         check=True, capture_output=True, text=True).stdout.strip()
    dest = pathlib.Path(args.dest).expanduser()
    dest.mkdir(parents=True, exist_ok=True)
    targets = args.targets.split()

    print("reading module sources from the committed MODULE.bazel.lock")
    index = registry_archives(top)
    print(f"  {len(index)} registry module(s) download a GitHub archive")

    overrides: dict[str, str] = {}
    write_block(bazelrc, [])  # start from what Bazel does unaided
    unfixable: set[str] = set()
    for round_ in range(1, MAX_ROUNDS + 1):
        print(f"round {round_}: bazel fetch --keep_going {' '.join(targets)}")
        blocked = blocked_downloads(top, targets)
        new = [index[u] for u in sorted(blocked) if u in index and index[u]["name"] not in overrides]
        unfixable |= {u for u in blocked if u not in index}
        if not new:
            break
        for mod in new:
            try:
                overrides[mod["name"]] = (
                    f"common --override_module={mod['name']}={prepare(mod, dest)}")
            except (RuntimeError, subprocess.CalledProcessError) as e:
                print(f"  FAILED {mod['name']}: {e}", file=sys.stderr)
                unfixable.add(mod["url"])
        write_block(bazelrc, sorted(overrides.values()))
    else:
        print(f"⚠ stopped after {MAX_ROUNDS} rounds", file=sys.stderr)

    print(f"\n{len(overrides)} override(s) in {bazelrc}"
          + (": " + ", ".join(sorted(overrides)) if overrides else ""))
    if overrides:
        print("⛔ Bazel rewrites MODULE.bazel.lock while these are in force. Before committing:\n"
              "     git checkout -- MODULE.bazel.lock")
    if unfixable:
        print("⚠ still blocked (not a registry module archive, or its patches failed):",
              file=sys.stderr)
        for u in sorted(unfixable):
            print(f"     {u}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
