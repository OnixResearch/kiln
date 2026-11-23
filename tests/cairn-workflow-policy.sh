#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

expected_policy="kiln-default"

assert_kiln_policy() {
  local payload=$1
  local reported
  reported=$(printf '%s\n' "$payload" | jq -r '.policy // empty')
  if [[ "$reported" != "$expected_policy" ]]; then
    printf 'refusing cairn output: expected policy %s, got %s\n' \
      "$expected_policy" "${reported:-<missing>}" >&2
    return 1
  fi
}

cairn policy export --source cairn-policy/default.ncl \
  --output cairn-policy/generated/cairn-policy.json --check

# Plain `cairn validate --root .` (no --policy flag) must resolve to Kiln's
# pinned policy (`kiln-default`) regardless of outer CAIRN_POLICY.
validate_output=$(cairn validate --root .)
assert_kiln_policy "$validate_output"
printf '%s\n' "$validate_output" |
  jq -e '.valid == true and .policy == "kiln-default"' >/dev/null

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Negative guard check: refuse a Cairn run whose reported policy is not Kiln's.
jq '.name = "lattice-default"' cairn-policy/generated/cairn-policy.json >"$tmp/foreign-policy.json"
foreign_output=$(cairn validate --root . --policy "$tmp/foreign-policy.json")
if assert_kiln_policy "$foreign_output" 2>"$tmp/guard-error"; then
  printf 'policy guard accepted a non-kiln-default Cairn run\n' >&2
  exit 1
fi
guard_error=$(<"$tmp/guard-error")
if [[ "$guard_error" != *'expected policy kiln-default, got lattice-default'* ]]; then
  printf 'policy guard failed for an unexpected reason: %s\n' "$guard_error" >&2
  exit 1
fi

# Self-contained live fixture so archiving repository changes never breaks this check.
mkdir -p "$tmp/fixture/.cairn/specs" "$tmp/fixture/.cairn/changes/policy-fixture"
cat >"$tmp/fixture/.cairn/changes/policy-fixture/metadata.json" <<'EOF'
{
  "workflow_profile": {
    "id": "spec-driven",
    "source": "change_create",
    "version": 1
  }
}
EOF
cat >"$tmp/fixture/.cairn/changes/policy-fixture/proposal.md" <<'EOF'
# Verify workflow policy fixture

## Why

Keep workflow policy verification independent of archived repository changes so
the spec-driven profile gate and negative policy checks stay reproducible.

## What Changes

- Exercise the `spec-driven` proposal gate against a live self-contained fixture.

## Impact

- **Files**: temporary fixture directory only.
- **Testing**: `tests/cairn-workflow-policy.sh`.
- **Out of scope**: repository source or lifecycle mutations.
EOF

proposal_output=$(cairn gate proposal policy-fixture --root "$tmp/fixture")
assert_kiln_policy "$proposal_output"
printf '%s\n' "$proposal_output" |
  jq -e '.valid == true and .verdict == "PASS" and .workflow_profile == "spec-driven" and .workflow_profile_compatibility_fallback == false' >/dev/null

nickel export cairn-policy/fixtures/negative-missing-outcome-profile.ncl >"$tmp/missing-outcome.json"
if cairn gate proposal policy-fixture --root "$tmp/fixture" \
  --policy "$tmp/missing-outcome.json" >"$tmp/output" 2>"$tmp/error"; then
  printf 'policy without outcome-machine passed a Cairn gate\n' >&2
  exit 1
fi
error=$(<"$tmp/error")
if [[ "$error" != *'initial workflow profile is missing: outcome-machine'* ]]; then
  printf 'missing outcome-machine profile failed for a different reason: %s\n' "$error" >&2
  exit 1
fi
printf 'Cairn accepts kiln-default spec-driven policy, denies foreign policies, and denies missing outcome-machine\n'
