#!/bin/sh
# Workspace provider profiles (#82): positive fixtures export, and each negative
# fixture is refused for its own reason rather than any contract failure.
# r[verify kiln.workspace_provider.evaluation]
# r[verify kiln.workspace_provider.evaluation.scenario.refused]
# r[verify kiln.workspace_provider.evaluation.scenario.claims]
set -eu
cd "$(dirname "$0")/.."

for fixture in config/fixtures/workspace-provider-*-positive.ncl; do
  if ! nickel export "$fixture" >/dev/null; then
    printf 'positive workspace provider profile failed: %s\n' "$fixture" >&2
    exit 1
  fi
done

refused() {
  fixture="config/fixtures/negative-workspace-provider-$1.ncl"
  if output=$(nickel export "$fixture" 2>&1); then
    printf 'negative workspace provider profile exported: %s\n' "$fixture" >&2
    exit 1
  fi
  case "$output" in
    *"$2"*) ;;
    *)
      printf '%s was refused without "%s":\n%s\n' "$fixture" "$2" "$output" >&2
      exit 1
      ;;
  esac
}

refused branch-view-boundary "whole-disk credential can reach data outside its view"
refused native-bucket-checkpoints "native-format bucket disks support neither checkpoints nor branches"
refused bucket-copy-acknowledgement "bucket_copy_is_acknowledgement"
refused forced-takeover-retry "forced_takeover_is_retry"
refused vendor-claim-qualified "only an adapter execution can qualify"
refused shared-attached-disk "writable_attempts"
refused provider-canonical-identity "canonical_algorithm"
refused undeletable-branches-retained "require a disposable-disk lifecycle"
refused zero-attempts "max_attempts"
refused local-credential "local materialization holds no provider credential"
printf 'workspace provider profiles: positives export, negatives refused for their own reasons\n'
