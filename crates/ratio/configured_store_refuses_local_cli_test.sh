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

# A real local book makes these failures meaningful: the operator verbs would
# otherwise refuse because no book or proposal exists. The diagnostic must
# name the configured backend, and no authoritative local bytes may change.
book="${root}/operator-book"
"$ratio" init --book "$book" >"${root}/seed.out" 2>&1
cp -R "$book" "${root}/before-operator-verbs"

refuse_writer() {
  label="$1"
  shift
  if RATIO_JOURNAL_LOCAL="${root}/not-a-directory" \
      "$ratio" "$@" --book "$book" \
      </dev/null >"${root}/${label}.out" 2>&1; then
    echo "$label acknowledged a write with unusable configured storage" >&2
    exit 1
  fi
  if ! grep -F 'Not a directory' "${root}/${label}.out" >/dev/null; then
    echo "$label refused for a reason other than the configured backend" >&2
    cat "${root}/${label}.out" >&2
    exit 1
  fi
  if ! diff -r "${root}/before-operator-verbs" "$book" >/dev/null; then
    echo "$label changed local book bytes despite unusable configured storage" >&2
    exit 1
  fi
}

refuse_writer post post "${root}/unused-events.json"
refuse_writer strike strike
refuse_writer close close --through 2026-06-30
refuse_writer approve approve missing-proposal
refuse_writer accept accept missing-break --because reviewed
