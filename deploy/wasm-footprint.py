#!/usr/bin/env python3
"""Add macOS peak physical footprint to an unchanged core measurement proto."""
import pathlib
import platform
import re
import shutil
import subprocess
import sys


def varint(value):
    result = bytearray()
    while value >= 128:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def main():
    if platform.system() != "Darwin" or len(sys.argv) != 6:
        raise SystemExit("macOS usage: wasm-footprint.py MODULE REVISION NEW_OUTPUT.pb HEIGHT VIEWS")
    module, revision, raw_output, height, views = sys.argv[1:]
    output = pathlib.Path(raw_output)
    core = output.with_suffix(".core.pb")
    log = output.with_suffix(".log")
    time_log = output.with_suffix(".time.log")
    if any(path.exists() for path in [output, core, log, time_log]):
        raise SystemExit("refusing to replace measurement evidence")
    node = shutil.which("node")
    if not node:
        raise SystemExit("Node.js is required")
    operator = pathlib.Path(__file__).with_name("wasm-capacity.cjs")
    with log.open("xb") as stdout, time_log.open("xb") as stderr:
        subprocess.run([
            "/usr/bin/time", "-l", node, str(operator), module, revision,
            str(core), height, views,
        ], stdout=stdout, stderr=stderr, check=True)
    matches = re.findall(r"^\s*(\d+)\s+peak memory footprint\s*$", time_log.read_text(), re.M)
    if len(matches) != 1:
        raise SystemExit("time did not report one peak physical footprint; raw evidence retained")
    peak = int(matches[0])
    if peak <= 0:
        raise SystemExit("invalid physical footprint")
    method = b"macOS /usr/bin/time -l: peak memory footprint (bytes), not RSS"
    # Fields 13 and 14 extend ratio.storage.v1.WasmFoldMeasurement. Preserve
    # the exact child protobuf separately; never rewrite an earlier attempt.
    augmented = core.read_bytes() + varint(13 * 8) + varint(peak)
    augmented += varint(14 * 8 + 2) + varint(len(method)) + method
    with output.open("xb") as target:
        target.write(augmented)
    print(f"peak physical footprint: {peak} bytes; {output}")


if __name__ == "__main__":
    main()
