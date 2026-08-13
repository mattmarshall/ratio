#!/usr/bin/env bash
#
# Run one of console/scripts/*.py under Bazel. Same shape as
# proto/mirrors_test.sh, and for the same reason: `sh_test` is the rule that can
# take a python3 from the environment without dragging a Python toolchain into a
# build whose only other Python is three test scripts.
set -euo pipefail
exec python3 "$@"
