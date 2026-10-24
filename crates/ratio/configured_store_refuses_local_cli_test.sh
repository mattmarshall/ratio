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

# These entry points create a new book; their failure must still name storage.
for operation in gen kind; do
  target="${root}/${operation}-new"
  if [[ "$operation" == gen ]]; then
    command_args=(gen --securities 1 --lots-per 1)
  else
    command_args=(init --kind personal)
  fi
  if RATIO_JOURNAL_LOCAL="${root}/not-a-directory" "$ratio" "${command_args[@]}" --book "$target" >"${root}/${operation}.out" 2>&1; then
    echo "$operation bypassed configured storage" >&2
    exit 1
  fi
  grep -F 'Not a directory' "${root}/${operation}.out" >/dev/null
  test ! -f "$target/journal.jsonl"
done

# Configured filesystem storage receives the journal; unset still writes JSONL.
RATIO_JOURNAL_LOCAL="${root}/objects" "$ratio" gen --securities 1 --lots-per 1 --book "${root}/generated" >"${root}/generated.out" 2>&1
test -f "${root}/objects/generated/journal/00000000000000000001"
test ! -s "${root}/generated/journal.jsonl"
RATIO_JOURNAL_LOCAL="${root}/objects" "$ratio" balance --book "${root}/generated" >"${root}/generated-balance.out" 2>&1
"$ratio" gen --securities 1 --lots-per 1 --book "${root}/local-generated" >"${root}/local-generated.out" 2>&1
test -s "${root}/local-generated/journal.jsonl"
"$ratio" balance --book "${root}/local-generated" >"${root}/local-balance.out" 2>&1
# The display includes the book path; compare the figures below its header.
tail -n +2 "${root}/generated-balance.out" >"${root}/remote-figures"
tail -n +2 "${root}/local-balance.out" >"${root}/local-figures"
diff -u "${root}/local-figures" "${root}/remote-figures"
