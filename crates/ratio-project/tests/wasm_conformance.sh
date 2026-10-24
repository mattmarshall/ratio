#!/usr/bin/env bash
set -euo pipefail
"$1" > "$TEST_TMPDIR/native-fixture.json"
node "$2" "$3" "$TEST_TMPDIR/native-fixture.json"
