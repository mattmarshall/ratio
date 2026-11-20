#!/usr/bin/env python3
"""The agent skills say what they claim, and AGENTS.md indexes all of them.

Run as //:agent_skills_test with every file under .agents/ and AGENTS.md as
arguments.

⛔ A SKILL NOTHING CHECKS WILL DRIFT. Each SKILL.md promises scripts by path and
is found by harnesses through its frontmatter. This holds, for every skill:

  1. the frontmatter is valid Agent Skills: `name` equals the directory,
     lowercase-hyphenated, at most 64 characters; `description` is present
     and at most 1024 characters
  2. AGENTS.md links to it, and AGENTS.md links to no skill that is gone
  3. every `scripts/<file>` it mentions exists, is executable, and parses
     (`bash -n` for shell, compile for Python)
  4. every relative link from a SKILL.md or AGENTS.md into .agents/ resolves

⚠ WHAT IT CANNOT CHECK is that a command in a SKILL.md still does what the
prose says. Each script was run when its skill was written; run it again when
you change what it drives.

⚠ A PERMISSION-ONLY CHANGE CAN PASS FROM CACHE. Bazel keys a test on file
contents, not mode bits, so `chmod -x` alone replays the last result. The
check does catch it (`--nocache_test_results` shows the failure), and git
records the mode (100755), so it is visible in review.
"""

import os
import pathlib
import py_compile
import re
import subprocess
import sys
import tempfile

fails: list[str] = []


def err(where: str, msg: str) -> None:
    fails.append(f"{where}: {msg}")


def frontmatter(text: str) -> dict[str, str] | None:
    m = re.match(r"^---\n(.*?)\n---\n", text, re.S)
    if not m:
        return None
    out = {}
    for line in m.group(1).splitlines():
        key, sep, value = line.partition(":")
        if sep:
            out[key.strip()] = value.strip()
    return out


def check_script(where: str, script: pathlib.Path, rel: str) -> None:
    """A promised script exists, is executable, and parses."""
    if not script.is_file():
        err(where, f"mentions {rel}, which does not exist")
        return
    if not os.access(script, os.X_OK):
        err(where, f"{rel} is not executable")
    if script.suffix == ".sh":
        r = subprocess.run(["bash", "-n", str(script)], capture_output=True, text=True)
        if r.returncode != 0:
            err(where, f"{rel} does not parse: {r.stderr.strip()}")
    else:
        with tempfile.TemporaryDirectory() as tmp:
            try:
                py_compile.compile(str(script), cfile=os.path.join(tmp, "x.pyc"), doraise=True)
            except py_compile.PyCompileError as e:
                err(where, f"{rel} does not compile: {e.msg}")


def main(argv: list[str]) -> int:
    paths = [pathlib.Path(a) for a in argv[1:]]
    agents_md = next((p for p in paths if p.name == "AGENTS.md" and ".agents" not in p.parts), None)
    if agents_md is None:
        print("AGENTS.md was not passed in")
        return 1
    root = agents_md.parent
    skills_dir = root / ".agents" / "skills"
    skills = sorted(d for d in skills_dir.iterdir() if d.is_dir()) if skills_dir.is_dir() else []
    if not skills:
        err(".agents/skills", "no skills found — this test is checking nothing")

    agents_text = agents_md.read_text()
    for skill in skills:
        where = f".agents/skills/{skill.name}"
        md = skill / "SKILL.md"
        if not md.is_file():
            err(where, "has no SKILL.md")
            continue
        text = md.read_text()

        # 1. Frontmatter.
        fm = frontmatter(text)
        if fm is None:
            err(where, "SKILL.md does not open with --- frontmatter ---")
        else:
            name, desc = fm.get("name", ""), fm.get("description", "")
            if name != skill.name:
                err(where, f"frontmatter name {name!r} is not the directory name")
            if not re.fullmatch(r"[a-z0-9]+(-[a-z0-9]+)*", name) or len(name) > 64:
                err(where, f"name {name!r} is not lowercase-hyphenated within 64 characters")
            if not 0 < len(desc) <= 1024:
                err(where, f"description is {len(desc)} characters; it must be 1..1024")

        # 2. Indexed by AGENTS.md.
        if f".agents/skills/{skill.name}/SKILL.md" not in agents_text:
            err("AGENTS.md", f"does not link to the {skill.name} skill")

        # 3a. A bare `scripts/x` is this skill's own. ⚠ Only bare: the
        # lookbehind keeps `console/scripts/fields_test.py` and
        # `.github/scripts/...` — other trees' scripts — out of it.
        for rel in sorted(set(re.findall(r"(?<![\w/.-])scripts/[\w.-]+\.(?:sh|py)\b", text))):
            check_script(where, skill / rel, rel)

    # 3b. A qualified `.agents/skills/<name>/scripts/x`, from any skill or from
    # AGENTS.md, is checked against the skill it names.
    for doc in [agents_md, *(s / "SKILL.md" for s in skills if (s / "SKILL.md").is_file())]:
        for name, rel in sorted(set(re.findall(
                r"\.agents/skills/([a-z0-9-]+)/(scripts/[\w.-]+\.(?:sh|py))\b", doc.read_text()))):
            check_script(str(doc.relative_to(root)), skills_dir / name / rel, f"{name}/{rel}")

    for name in sorted(set(re.findall(r"\.agents/skills/([a-z0-9-]+)/SKILL\.md", agents_text))):
        if not (skills_dir / name / "SKILL.md").is_file():
            err("AGENTS.md", f"links to the {name} skill, which does not exist")

    # 4. Relative links into .agents/ resolve.
    for doc in [agents_md, *(s / "SKILL.md" for s in skills if (s / "SKILL.md").is_file())]:
        for link in re.findall(r"\]\(([^)#\s]+)(?:#[^)]*)?\)", doc.read_text()):
            if link.startswith(("http", "mailto:")):
                continue
            # normpath, not resolve(): under Bazel every file is a symlink into
            # the source tree, and following it would check the wrong tree.
            target = pathlib.Path(os.path.normpath(doc.parent / link))
            if ".agents" in target.parts and not target.exists():
                err(str(doc.relative_to(root)), f"links to {link}, which does not exist")

    if fails:
        print("\n".join(fails))
        return 1
    print(f"{len(skills)} skill(s) checked")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
