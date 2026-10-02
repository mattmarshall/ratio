#!/usr/bin/env bash
set -euo pipefail

ratio="$1"
root="$(mktemp -d "${TEST_TMPDIR}/configured-store-XXXXXX")"
printf 'occupied\n' > "${root}/not-a-directory"

# ⛔ A configured but unusable object backend must refuse before a CLI or MCP
# command can acknowledge writes in an unrelated local book directory.
for command in init mcp; do
  if RATIO_JOURNAL_LOCAL="${root}/not-a-directory" \
      "$ratio" "$command" --book "${root}/${command}-book" \
      </dev/null >"${root}/${command}.out" 2>&1; then
    echo "$command accepted a local-only book with unusable configured storage" >&2
    exit 1
  fi
  if [[ -f "${root}/${command}-book/journal.jsonl" ]]; then
    echo "$command wrote a local journal despite unusable configured storage" >&2
    exit 1
  fi
done
