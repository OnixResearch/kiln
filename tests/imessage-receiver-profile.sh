#!/bin/sh
# iMessage receiver contract (#71): the positive profile exports, and each
# negative profile is refused by the field it violates. The receiver itself is
# a documented no-go; only its contract is kept.
# r[verify kiln.imessage.receiver] r[verify kiln.imessage.receiver.contract]
set -eu
cd "$(dirname "$0")/.."

if ! nickel export config/fixtures/imessage-receiver-positive.ncl >/dev/null; then
  printf 'positive iMessage receiver profile failed\n' >&2
  exit 1
fi

refused() {
  fixture="config/fixtures/negative-imessage-receiver-$1.ncl"
  if output=$(nickel export "$fixture" 2>&1); then
    printf 'negative iMessage receiver profile exported: %s\n' "$fixture" >&2
    exit 1
  fi
  case "$output" in
    *"contract broken by the value of \`$2\`"*) ;;
    *)
      printf '%s was refused without naming %s:\n%s\n' "$fixture" "$2" "$output" >&2
      exit 1
      ;;
  esac
}

refused dynamic-destination dynamic_destination
refused inline-secret inline_secret
refused message-field message_fields
refused over-message-bytes max_message_bytes
refused zero-message-bytes max_message_bytes
refused over-run-url-bytes max_run_url_bytes
refused zero-run-url-bytes max_run_url_bytes
refused payload-schema payload_schema
refused recipient-handle recipient_handles
refused run-url-base run_url_base
printf 'iMessage receiver profiles: positive exports, negatives refused by their fields\n'
