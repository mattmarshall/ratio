#!/usr/bin/env bash
# Parse the established seeded-rail list without ever printing a rejected value.
set -euo pipefail

value="${1-}"
if [[ -z "$value" || "$value" == *$'\n'* || "$value" == *$'\r'* || "$value" == *'\'* || "$value" == ,* || "$value" == *, || "$value" == *,,* ]]; then
  echo "invalid DEMO_MEMBERS entry" >&2
  exit 1
fi

IFS=',' read -r -a members <<< "$value"
subjects=()
for member in "${members[@]}"; do
  if [[ "$member" =~ ^user_[A-Za-z0-9_]+$ ]]; then
    subjects+=("$member")
  elif [[ "$member" =~ ^[^,@[:space:]]+@[^,@[:space:]]+$ ]]; then
    : # A verified email is an existing seeded-rail member, not the control principal.
  else
    echo "invalid DEMO_MEMBERS entry" >&2
    exit 1
  fi
done

if [[ "${#subjects[@]}" -ne 1 ]]; then
  echo "DEMO_MEMBERS must name exactly one AuthKit subject" >&2
  exit 1
fi
printf '%s' "${subjects[0]}"
