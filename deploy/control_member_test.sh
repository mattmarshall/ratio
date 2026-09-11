#!/usr/bin/env bash
set -euo pipefail

parser="$TEST_SRCDIR/_main/deploy/control_member.sh"

for value in \
  user_abc \
  user_abc,person@example.com \
  person@example.com,user_abc
do
  [[ "$($parser "$value")" == user_abc ]]
done

bad_values=(
  person@example.com
  user_one,user_two
  'user_one,'
  'user_one, '
  ,user_one
  user_one,,person@example.com
  'user_\one'
  'a@b@c,user_one'
  'user_one,a@b'
  'user_one,a@-b.example'
  $'user_one,a\001b@example.com'
  'bad,user_one'
)
for value in "${bad_values[@]}"; do
  if "$parser" "$value" >/dev/null 2>/dev/null; then
    echo "accepted malformed member shape" >&2
    exit 1
  fi
done

if "$parser" $'user_one\njunk' >/dev/null 2>/dev/null; then
  echo "accepted multiline member shape" >&2
  exit 1
fi

echo "ok  control member parser validates the complete value"
