"""Every failure-path probe must go red for the reason it names.

A probe is a `tla_check` tagged `manual` whose whole purpose is to FAIL: one
dial flipped, one invariant that must be violated. Running it reports FAILED,
and a human reads that as success.

⛔ WHICH MEANS EVERY OTHER WAY OF FAILING LOOKS EXACTLY LIKE WORKING. TLC exits
non-zero for a violated invariant, for a missing constant, for a parse error,
for a module it cannot find. Adding a `CONSTANT` to `ReliefEngine.tla` made
`StaleFactorRelief.cfg` fail with "the constant parameter MaxConfigs is not
assigned a value" — still red, still "passing" as a probe, and no longer
checking anything at all.

This is the static half of the guard, and it runs in CI. It does not run TLC —
it checks that each probe COULD say what it claims:

  1. the cfg assigns every CONSTANT its spec declares, so the run reaches the
     model rather than dying in configuration
  2. the cfg header names the invariant that must go red, in the house form
     `\\* <Name> MUST go red.`
  3. that invariant is listed under the cfg's INVARIANTS, so TLC is actually
     asked about it
  4. and it is defined in the spec, so the name is not a typo that would parse
     as an unknown operator

⚠ WHAT IT CANNOT CHECK is that the flipped dial still causes the violation —
only TLC can, and that is `probes.sh`. A dial that stopped mattering would pass
every check here.
"""

import pathlib
import re
import sys

fails = []


def err(where, msg):
    fails.append(f"{where}: {msg}")


def constants_of(spec_text):
    """The CONSTANTS a spec declares.

    The block runs from `CONSTANTS` to the first blank line. Names are
    comma-separated and each may carry a trailing `\\*` comment.
    """
    m = re.search(r"^CONSTANTS?\b(.*?)\n\s*\n", spec_text, re.M | re.S)
    if not m:
        return set()
    names = set()
    for line in m.group(1).splitlines():
        line = re.sub(r"\\\*.*$", "", line).strip().rstrip(",")
        for part in line.split(","):
            part = part.strip()
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", part):
                names.add(part)
    return names


def defined_operators(spec_text):
    return set(re.findall(r"^([A-Za-z_][A-Za-z0-9_]*)\s*==", spec_text, re.M))


def section(cfg_text, keyword):
    """The names listed under `keyword` — INVARIANTS or PROPERTIES."""
    m = re.search(rf"^{keyword}\b(.*?)(?=^[A-Z]+\b|\Z)", cfg_text, re.M | re.S)
    if not m:
        return set()
    out = set()
    for line in m.group(1).splitlines():
        line = re.sub(r"\\\*.*$", "", line).strip()
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", line):
            out.add(line)
    return out


def main(argv):
    tla_dir = pathlib.Path(argv[1]).parent
    build = pathlib.Path(argv[1]).read_text()

    # Every `tla_check` in the BUILD file, with its module, config and tags.
    checks = []
    for block in re.findall(r"tla_check\(\s*(.*?)\n\)", build, re.S):
        name = re.search(r'name\s*=\s*"([^"]+)"', block)
        module = re.search(r'module\s*=\s*"([^"]+)"', block)
        config = re.search(r'config\s*=\s*"([^"]+)"', block)
        if not (name and module and config):
            err("tla/BUILD.bazel", f"a tla_check is missing name/module/config: {block[:60]!r}")
            continue
        checks.append(
            (name.group(1), module.group(1), config.group(1), "manual" in block)
        )

    probes = [c for c in checks if c[3]]
    if not probes:
        err("tla/BUILD.bazel", "no manual-tagged probes found — this test is checking nothing")

    for name, module, config, _ in probes:
        mod_path = tla_dir / module
        cfg_path = tla_dir / config
        if not mod_path.is_file():
            err(name, f"module {module} does not exist")
            continue
        if not cfg_path.is_file():
            err(name, f"config {config} does not exist")
            continue

        # A probe module is a trampoline: `EXTENDS <Spec>`. The constants and
        # the operators live in the spec it extends.
        mod_text = mod_path.read_text()
        ext = re.search(r"^EXTENDS\s+([A-Za-z_][A-Za-z0-9_]*)", mod_text, re.M)
        if not ext:
            err(name, f"{module} does not EXTEND a spec")
            continue
        spec_path = tla_dir / f"{ext.group(1)}.tla"
        if not spec_path.is_file():
            err(name, f"{module} extends {ext.group(1)}, which does not exist")
            continue
        spec_text = spec_path.read_text()
        cfg_text = cfg_path.read_text()

        # 1. Every declared constant is assigned.
        declared = constants_of(spec_text)
        assigned = set(re.findall(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=", cfg_text, re.M))
        missing = sorted(declared - assigned)
        if missing:
            err(
                config,
                f"{ext.group(1)} declares {', '.join(missing)} and this cfg does not "
                f"assign {'them' if len(missing) > 1 else 'it'} — TLC dies in "
                "configuration, which exits exactly like a violated invariant",
            )

        # 2. The header names what must go red.
        claim = re.search(r"\\\*\s*([A-Za-z_][A-Za-z0-9_]*)\s+MUST go red\.", cfg_text)
        if not claim:
            err(
                config,
                "no header comment of the form `\\* <Invariant> MUST go red.` — a probe "
                "that does not say what it breaks cannot be checked, by this or by a "
                "reader",
            )
            continue
        wanted = claim.group(1)

        # 3. TLC is asked about it.
        listed = section(cfg_text, "INVARIANTS") | section(cfg_text, "PROPERTIES")
        if wanted not in listed:
            err(
                config,
                f"claims {wanted} must go red, but does not list it under INVARIANTS or "
                f"PROPERTIES — TLC is never asked. Listed: {sorted(listed)}",
            )

        # 4. And the name is real.
        if wanted not in defined_operators(spec_text):
            err(config, f"{wanted} is not defined in {ext.group(1)}.tla — a typo here "
                        "parses as an unknown operator, which is also a red run")

    for f in fails:
        print(f"  x {f}")
    if fails:
        print(f"\n{len(fails)} probe(s) could not say what they claim")
        return 1
    print(f"  ok  {len(probes)} probes, each assigning every constant and naming a "
          "live invariant it is asked about")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
