#!/usr/bin/env bash
# Operator recipe, not automatically executed. Run only on aspen1 under the
# parent-owned maintenance watchdog, as kilnqual20261106, after patched reboot,
# SMT/KSM disablement, service drain and exclusive CAT delegation.
# Parent owns building this Cargo package and staging its dynamic runtime closure.
set -euo pipefail
: "${QUAL_BINARY:?absolute path to the built kiln-aspen1-approved-consumer binary}"
: "${QUAL_SOURCE:?absolute readable native Git repository containing the approved exact Kiln commit}"
: "${QUAL_COMMIT:?full lower-case 40-character exact source commit}"
: "${QUAL_CAT:?existing operator-created and delegated resctrl CAT group name}"
: "${QUAL_OPERATOR_LABEL:?actual authorizing operator identity label from parent session}"
: "${QUAL_TOOL_PATH:?PATH containing immutable Nix-store systemd-run systemctl env}"
QUAL_GUEST=${QUAL_GUEST:-/nix/store/6rjx8bv22brsdha5vsp2mqv2qn75v7kz-kiln-microvm-guest}
QUAL_ROOT=${QUAL_ROOT:-/var/lib/kiln-qualification/aspen1-20261106-cf6e793b-r5/runtime}
QUAL_MODE=${QUAL_MODE:-run}
# The guest manifest supplies all image hashes/paths. Current parent-supplied
# manifest uses kernel 6.18.60 and Firecracker 1.16.1; no guessed store paths.
# Host runtime dependencies: /dev/kvm RW, /run/user/UID/systemd, proc/sysfs,
# readable booted host kernel package, resctrl L3 schemata/tasks delegation.
# Native Git uses kiln-adapter-git/libgit2; no Git CLI, fake source port, Web
# workflow port, provider credential, token file or source mutation is used.
# The root-owned immutable Git source requires an exact safe.directory entry in
# this disposable account's private Git configuration, never a wildcard.
export PATH="$QUAL_TOOL_PATH"
export XDG_RUNTIME_DIR="/run/user/$(id -u)"
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
exec "$QUAL_BINARY" "$QUAL_MODE" \
  --guest "$QUAL_GUEST" \
  --state "$QUAL_ROOT/approved-state" \
  --source "$QUAL_SOURCE" \
  --output "$QUAL_ROOT/approved-output" \
  --cat "$QUAL_CAT" \
  --commit "$QUAL_COMMIT" \
  --operator-label "$QUAL_OPERATOR_LABEL"
# Recovery: preserve every argument/value, state, keys and original source;
# set QUAL_MODE=recover and invoke this same recipe. Recovery never runs a new
# guest: it reads execution.get, polls the original attempt for <=120 seconds,
# then execution.recover reaps uncertainty rather than executing again.
# Repeated recovery keeps separately numbered evidence directories without
# changing the exact attempt identity. Never delete the original state to retry.
# If native acquisition failed before profile.json and attempt.json existed,
# QUAL_MODE=resume-bootstrap resumes the original binding/keys/review/price only;
# it refuses any initialized attempt. This mode was exercised after Git trust
# configuration was supplied, without deleting original state or keys.
# Stop the process/owned units through the parent's watchdog on an outer deadline;
# retain state for recovery and do not label interrupted qualification as passed.
# On success: retain recipes, binding.json, native-source.json, reviewed pipeline,
# operator-authorization.json, all request/response pairs, receipt.json, verdict.
# Never export operator.key/server.key or the raw hosted database as public evidence.
# Parent restores services/temp settings and removes owned runtime/source assets
# only after proof has been retained and owned VM/process teardown inspected.
