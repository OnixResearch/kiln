#!/usr/bin/env bash
set -euo pipefail

# Refresh the upstream-derived files of the vendored Radicle canonical module.
#
# The crate `vendor/heartwood-canonical` contains:
#  - nine canonical algorithm files and the DID wrapper, extracted verbatim from
#    heartwood at the pinned commit (two files have fixture-dependent test
#    modules removed; see `stripped_test_modules`);
#  - the upstream `LICENSE-MIT` and `LICENSE-APACHE` texts, extracted verbatim
#    from the repository root at the pinned commit;
#  - hand-authored shim files (`src/lib.rs`, `src/git.rs`, `src/git/raw.rs`,
#    `src/identity.rs`, `src/identity/doc.rs`) that reproduce the upstream module
#    paths the algorithm uses without pulling the sqlite-dependent node crate.
#
# This script refreshes only the upstream-derived files, verifies every derived
# file digest against `vendor/heartwood-canonical.source.json`, and refuses to
# run when the shim files are absent. A checkout is only an input; read pinned
# Git objects, never a mutable working tree.
if (( $# != 1 )); then
  printf 'usage: %s <heartwood-checkout>\n' "$0" >&2
  exit 2
fi
source_root=$1
source_commit=71a91042d96bb994c4cd4311d85f5cd1a6311b4e
repo_root=$(realpath "$(dirname "${BASH_SOURCE[0]}")/..")
destination="$repo_root/vendor/heartwood-canonical"
source_record="$repo_root/vendor/heartwood-canonical.source.json"

if [[ "$(jq -r '.source_commit' "$source_record")" != "$source_commit" ]]; then
  printf 'heartwood canonical source record disagrees with importer pin\n' >&2
  exit 1
fi
for shim in src/lib.rs src/git.rs src/git/raw.rs src/identity.rs src/identity/doc.rs; do
  if [[ ! -f "$destination/$shim" ]]; then
    printf 'missing hand-authored shim %s\n' "$shim" >&2
    exit 1
  fi
done
git -C "$source_root" cat-file -e "${source_commit}^{commit}"

temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
mkdir -p "$temporary/src/git/canonical" "$temporary/src/identity"
for file in \
  canonical.rs \
  canonical/convergence.rs \
  canonical/effects.rs \
  canonical/error.rs \
  canonical/protect.rs \
  canonical/quorum.rs \
  canonical/rules.rs \
  canonical/symbolic.rs \
  canonical/voting.rs
do
  git -C "$source_root" show "${source_commit}:crates/radicle/src/git/$file" > "$temporary/src/git/$file"
done
git -C "$source_root" show "${source_commit}:crates/radicle/src/identity/did.rs" > "$temporary/src/identity/did.rs"
for license in LICENSE-MIT LICENSE-APACHE; do
  git -C "$source_root" show "${source_commit}:$license" > "$temporary/$license"
done

python3 - "$temporary" "$source_record" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
record = json.loads(Path(sys.argv[2]).read_text())


def strip_module(text: str, marker: str, label: str) -> str:
    start = text.index(marker)
    body_start = text.index("{", start)
    depth = 0
    end = None
    for index in range(body_start, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    if end is None or text[end:].strip():
        raise SystemExit(f"unexpected trailing bytes after {label} test module")
    return text[:start].rstrip() + "\n"


canonical = root / "src/git/canonical.rs"
canonical.write_text(strip_module(
    canonical.read_text(),
    "#[cfg(test)]\n#[allow(clippy::unwrap_used)]\nmod tests {",
    "canonical.rs",
))
symbolic = root / "src/git/canonical/symbolic.rs"
symbolic.write_text(strip_module(
    symbolic.read_text(),
    "#[cfg(test)]\n#[allow(clippy::unwrap_used)]\nmod test {",
    "symbolic.rs",
))
rules = root / "src/git/canonical/rules.rs"
rules_text = rules.read_text()
declaration = "#[cfg(test)]\nmod test;\n\n"
if rules_text.count(declaration) != 1:
    raise SystemExit("rules.rs test declaration context changed upstream")
rules.write_text(rules_text.replace(declaration, ""))

for relative, expected in record["derived_files"].items():
    digest = hashlib.sha256((root / relative).read_bytes()).hexdigest()
    if digest != expected:
        raise SystemExit(f"derived file digest mismatch for {relative}: {digest}")
PY

for relative in $(jq -r '.derived_files | keys[]' "$source_record"); do
  cp "$temporary/$relative" "$destination/$relative"
done
printf 'refreshed heartwood canonical files at %s\n' "$source_commit"
