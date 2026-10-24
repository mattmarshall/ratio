#!/usr/bin/env python3
"""Fail a Bazel build when a production ObjectStore escapes conformance review."""

import pathlib
import re
import sys


EXPECTED = {
    ("crates/ratio-store/src/objects.rs", "Arc"),
    ("crates/ratio-store/src/objects.rs", "MemoryStore"),
    ("crates/ratio-store/src/objects.rs", "DirStore"),
    ("crates/ratio/src/scale.rs", "S3"),
    ("crates/ratio-postgres-store/src/lib.rs", "PgStore"),
    # Manual provider wrappers delegate conditional writes unchanged; they
    # expose counters and the before-height curve, not another backend.
    ("crates/ratio/src/checkpoint_live.rs", "Counted"),
    ("crates/ratio/src/height_live.rs", "Listed"),
    # Test-only doubles also belong here: a new implementation must be
    # classified explicitly instead of disappearing after a cfg(test) marker.
    # Existing #300 fail-closed and #358 direct-height regression doubles.
    ("crates/ratio-store/src/changes.rs", "UnavailableStore"),
    ("crates/ratio-store/src/proposals.rs", "UnavailableStore"),
    ("crates/ratio-store/src/reports.rs", "UnavailableStore"),
    ("crates/ratio-store/src/objects.rs", "MaxOnlyStore"),
    ("crates/ratio-store/src/objects.rs", "BrokenScan"),
    ("crates/ratio-store/src/lib.rs", "CountingStore"),
    ("crates/ratio-store/src/lib.rs", "DelayedGetStore"),
    ("crates/ratio-store/src/control.rs", "FaultStore"),
    ("crates/ratio-console/src/bootstrap_tests.rs", "CountControlScans"),
    ("crates/ratio-console/src/bootstrap_tests.rs", "FaultStore"),
}
PATTERN = re.compile(
    r"impl(?:<[^\n]*>)?\s+(?:ratio_store::)?ObjectStore\s+for\s+(\w+)"
)


def main() -> None:
    found = set()
    fixture = ""
    adapter = ""
    native = ""
    workflow = ""
    for raw in sys.argv[1:]:
        path = pathlib.Path(raw)
        normalized = path.as_posix()
        if normalized.endswith("crates/ratio-project/tests/backend_conformance.rs"):
            fixture = path.read_text()
        if normalized.endswith("crates/ratio/src/scale.rs"):
            adapter = path.read_text()
        source = path.read_text()
        if normalized.endswith("crates/ratio-postgres-store/tests/conformance.rs"):
            native = source
        if normalized.endswith(".github/workflows/build.yml"):
            workflow = source
        if "crates/" not in normalized:
            continue
        relative = "crates/" + normalized.split("crates/", 1)[1]
        found.update((relative, match.group(1)) for match in PATTERN.finditer(source))

    if found != EXPECTED:
        missing = sorted(EXPECTED - found)
        extra = sorted(found - EXPECTED)
        raise SystemExit(
            f"ObjectStore implementations changed: missing={missing}, extra={extra}. "
            "Extend backend_conformance_gate with a fixture for each new backend."
        )
    if "backend_fixture::exercise" not in native or "//crates/ratio-postgres-store:pg_conformance_test" not in workflow:
        raise SystemExit("Native Postgres conformance must use the reusable fixture and run in required live CI")
    for required in ("MemoryStore::new", "DirStore::at", "MemoryStore::unconditional"):
        if required not in fixture:
            raise SystemExit(f"backend conformance fixture lost {required}")
    for required in (
        "s3_preserves_binary_objects_and_the_conditional_publication_header",
        "s3_adapter_runs_the_same_journal_and_side_plane_fixture",
        "backend_fixture::exercise",
    ):
        if required not in adapter:
            raise SystemExit(f"S3 adapter conformance lost {required}")


if __name__ == "__main__":
    main()
