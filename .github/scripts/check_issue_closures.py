#!/usr/bin/env python3
"""Refuse accidental GitHub issue-closing directives in PRs and commits.

This is a deliberately stricter authoring convention than GitHub's parser,
not a claim to infer whether the work actually meets an issue's acceptance.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

KEYWORD = r"(?:close[sd]?|fix(?:es|ed)?|resolve[sd]?)"
REFERENCE = r"(?:https://github\.com/[\w.-]+/[\w.-]+/issues/[1-9]\d*|(?:[\w.-]+/[\w.-]+)?#[1-9]\d*)"
DIRECTIVE = re.compile(rf"\b{KEYWORD}\s*:?[ \t\r\n]+(?P<issue>{REFERENCE})", re.I)
COMPLETE_LINE = re.compile(rf"{KEYWORD}[ \t]*:?[ \t]+{REFERENCE}[.!]?", re.I)
REFERENCE_RE = re.compile(REFERENCE, re.I)
REMAINS = re.compile(r"^\s*(?:[-*]\s+)?(?:Related|Remaining work)\s*:", re.I)


def issue_key(reference, repository):
    """Unify local, qualified, and URL references before checking conflicts."""
    if reference.lower().startswith("https://github.com/"):
        repository, number = reference[len("https://github.com/"):].rsplit("/issues/", 1)
    else:
        qualified, number = reference.rsplit("#", 1)
        repository = qualified or repository
    return repository.lower(), number


def inspect_text(text, source, repository, allow_closing=True):
    errors = []
    closures = set()
    remaining = set()
    lines = text.splitlines(keepends=True)
    contexts = []
    offset = 0
    fence = None
    in_comment = False
    for number, line in enumerate(lines, 1):
        stripped = line.strip()
        marker = re.match(r"^(`{3,}|~{3,})", stripped)
        quoted = fence is not None or in_comment or "<!--" in line
        if marker:
            quoted = True
            if fence is None:
                fence = marker.group(1)[0]
            elif marker.group(1)[0] == fence:
                fence = None
        if "<!--" in line:
            in_comment = True
        if "-->" in line:
            in_comment = False
        contexts.append((offset, offset + len(line), number, stripped, quoted))
        if REMAINS.match(line):
            remaining.update(issue_key(m.group(), repository) for m in REFERENCE_RE.finditer(line))
        offset += len(line)

    # Scan across newlines too: wrapping a negation must not hide its keyword.
    for match in DIRECTIVE.finditer(text):
        start, end, number, line, quoted = next(c for c in contexts if c[0] <= match.start() < c[1])
        neighbors = [contexts[i][3] for i in (number - 2, number) if 0 <= i < len(contexts)]
        # A newline after "does not" is prose wrapping, not a completion line.
        isolated = all(not value or COMPLETE_LINE.fullmatch(value) or REMAINS.match(value) for value in neighbors)
        if not allow_closing or quoted or not isolated or match.end() > end or not COMPLETE_LINE.fullmatch(line):
            errors.append(f"{source}:{number}: closing keyword must be an unquoted standalone completion line; use 'Related: #N' or 'Remaining work: #N' for partial work")
        else:
            closures.add(issue_key(match.group("issue"), repository))
    return errors, closures, remaining


def check_documents(documents, repository):
    errors = []
    closed = set()
    remaining = set()
    for source, text, allow_closing in documents:
        found, closes, stays = inspect_text(text, source, repository, allow_closing)
        errors.extend(found)
        closed.update(closes)
        remaining.update(stays)
    for repo, number in sorted(closed & remaining):
        errors.append(f"{repo}#{number}: listed for both completion and related/remaining work; choose one disposition")
    return errors


def event_documents(path):
    event = json.loads(Path(path).read_text())
    pr = event["pull_request"]
    documents = [("PR title", pr.get("title") or "", False), ("PR body", pr.get("body") or "", True)]
    # SHAs come from event JSON, never from interpolated shell source.
    base, head = pr["base"]["sha"], pr["head"]["sha"]
    if not all(re.fullmatch(r"[a-f0-9]{40}", sha) for sha in (base, head)):
        raise ValueError("invalid PR commit SHA")
    messages = subprocess.run(
        ["git", "log", "--format=%B%x00", f"{base}..{head}"],
        check=True, capture_output=True, text=True,
    ).stdout
    for index, message in enumerate(messages.split("\0"), 1):
        if message.strip():
            documents.append((f"commit {index}", message, True))
    return documents, event["repository"]["full_name"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--event", help="GitHub pull_request event JSON; also checks every PR commit")
    inputs.add_argument("--text-file", help="locally check a PR body or commit message")
    parser.add_argument("--repository", default="mattmarshall/ratio")
    args = parser.parse_args()
    if args.event:
        documents, repository = event_documents(args.event)
    else:
        documents = [(args.text_file, Path(args.text_file).read_text(), True)]
        repository = args.repository
    errors = check_documents(documents, repository)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Issue closure wording checked. Review acceptance evidence before completing an issue.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
