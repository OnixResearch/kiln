#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

if ! grep -Fq 'cargo run --example issue_and_verify' README.md; then
  echo 'README must point to the runnable issue_and_verify happy-path example' >&2
  exit 1
fi

cargo check --example issue_and_verify
