#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly DEFAULT_SOURCE_REPO="${REPO_ROOT}/../trellis"
readonly VENDOR_DIR="${REPO_ROOT}/vendor/verified-logic"
readonly REVISION_FILE="${REPO_ROOT}/vendor/verified-logic-revision.txt"
readonly REQUIRED_PATHS=(Cargo.toml LICENSE src)
readonly OPTIONAL_PATHS=(README.md build.rs .cargo)

source_repo="${1:-${DEFAULT_SOURCE_REPO}}"

if ! git -C "${source_repo}" rev-parse --show-toplevel >/dev/null 2>&1; then
  fail "source repo '${source_repo}' is not a git checkout"
fi

source_revision="$(git -C "${source_repo}" rev-parse HEAD)"
source_remote="$(git -C "${source_repo}" config --get remote.origin.url || printf '%s' "${source_repo}")"
tracked_dirty_status="$(git -C "${source_repo}" status --porcelain --untracked-files=no)"

if [[ -n "${tracked_dirty_status}" ]]; then
  printf 'note: source repo has uncommitted tracked changes; importing committed HEAD %s only\n' "${source_revision}" >&2
fi

archive_paths=()
for path in "${REQUIRED_PATHS[@]}"; do
  if ! git -C "${source_repo}" cat-file -e "HEAD:${path}" 2>/dev/null; then
    fail "required path '${path}' is missing from ${source_repo} at ${source_revision}"
  fi
  archive_paths+=("${path}")
done

for path in "${OPTIONAL_PATHS[@]}"; do
  if git -C "${source_repo}" cat-file -e "HEAD:${path}" 2>/dev/null; then
    archive_paths+=("${path}")
  fi
done

rm -rf "${VENDOR_DIR}"
mkdir -p "${VENDOR_DIR}"
git -C "${source_repo}" archive --format=tar HEAD "${archive_paths[@]}" | tar -x -C "${VENDOR_DIR}"

mkdir -p "$(dirname -- "${REVISION_FILE}")"
{
  printf 'revision=%s\n' "${source_revision}"
  printf 'source_repo=%s\n' "${source_remote}"
} > "${REVISION_FILE}"

printf 'vendored verified-logic revision %s into %s\n' "${source_revision}" "${VENDOR_DIR}"
