#!/usr/bin/env bash
set -euo pipefail

ratio="$1"
root="${TEST_TMPDIR:?}/recovery-cli-$$"
source="$root/source"
backup="$root/backup"
restored="$root/restored"
mkdir -p "$source/_bootstrap/publications" "$source/alpha/journal"
printf 'bootstrap' > "$source/_bootstrap/publications/alpha"
printf 'journal entry' > "$source/alpha/journal/00000000000000000001"
printf 'not a directory' > "$root/unusable-configured-store"
export RATIO_JOURNAL_LOCAL="$root/unusable-configured-store"

if RATIO_RECOVERY_SOURCE_LOCAL="$source" RATIO_RECOVERY_BACKUP_LOCAL="$source/nested" \
  "$ratio" recovery capture > "$root/overlap.out" 2>&1; then
  echo 'capture accepted a nested backup namespace' >&2
  exit 1
fi
grep -F 'namespaces must be disjoint' "$root/overlap.out" >/dev/null

RATIO_RECOVERY_SOURCE_LOCAL="$source" RATIO_RECOVERY_BACKUP_LOCAL="$backup" \
  "$ratio" recovery capture > "$root/capture.out"
test -f "$backup/_recovery/capture.pb"
grep -F 'captured 2 objects' "$root/capture.out" >/dev/null
if RATIO_RECOVERY_SOURCE_LOCAL="$source" RATIO_RECOVERY_BACKUP_LOCAL="$backup" \
  "$ratio" recovery capture > "$root/second.out" 2>&1; then
  echo 'capture accepted a nonempty backup destination' >&2
  exit 1
fi
grep -F 'destination must be empty' "$root/second.out" >/dev/null

rm -rf "$source"
if RATIO_RECOVERY_BACKUP_LOCAL="$backup" RATIO_RECOVERY_DESTINATION_LOCAL="$backup/nested" \
  "$ratio" recovery restore > "$root/restore-overlap.out" 2>&1; then
  echo 'restore accepted a nested destination namespace' >&2
  exit 1
fi
grep -F 'namespaces must be disjoint' "$root/restore-overlap.out" >/dev/null
RATIO_RECOVERY_BACKUP_LOCAL="$backup" RATIO_RECOVERY_DESTINATION_LOCAL="$restored" \
  "$ratio" recovery restore > "$root/restore.out"
grep -F 'restored 2 objects' "$root/restore.out" >/dev/null
cmp "$backup/_bootstrap/publications/alpha" "$restored/_bootstrap/publications/alpha"
cmp "$backup/alpha/journal/00000000000000000001" \
  "$restored/alpha/journal/00000000000000000001"

printf 'changed' > "$backup/alpha/journal/00000000000000000001"
if RATIO_RECOVERY_BACKUP_LOCAL="$backup" RATIO_RECOVERY_DESTINATION_LOCAL="$root/rejected" \
  "$ratio" recovery restore > "$root/corrupt.out" 2>&1; then
  echo 'restore accepted changed backup bytes' >&2
  exit 1
fi
grep -F 'differs from manifest' "$root/corrupt.out" >/dev/null
