#!/bin/sh
# Invoked only inside the writable, disposable Nix build sandbox.
set -eu
[ -n "${NIX_BUILD_TOP:-}" ] || {
    echo 'Celld boundary gate requires an isolated Nix build sandbox' >&2
    exit 2
}
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT

for crate in kiln-core kiln-provider-api kiln-standalone; do
    cargo tree -p "$crate" --edges normal --locked --offline >"$tmp/$crate.tree"
done
check_tree() {
    crate=$1
    tree=$2
    if edge=$(grep -E 'kiln-adapter-celld|celld|workerd|wrangler|v8|object-store|object_store' "$tree"); then
        printf '%s: forbidden runtime dependency edge: %s\n' "$crate" "$edge" >&2
        return 1
    fi
}
check_tree kiln-core "$tmp/kiln-core.tree"
check_tree kiln-provider-api "$tmp/kiln-provider-api.tree"
check_tree kiln-standalone "$tmp/kiln-standalone.tree"

# r[verify kiln.celld.dependency_boundary.scenario.optional]
# Actual standalone/core/provider tests run without the Celld runtime or S3
# credentials; the three real dependency trees above exclude Worker/V8.
cargo test -p kiln-core -p kiln-provider-api -p kiln-standalone --lib --locked --offline

# r[verify kiln.celld.dependency_boundary.scenario.import]
# The negative case edits ONLY an independently copied disposable workspace.
# It both imports a real implementation type into core and resolves that edge
# through Cargo; the same gate must report the specific offending edge.
cp -R . "$tmp/workspace"
mkdir -p "$tmp/workspace/crates/celld-forbidden-runtime/src"
cat >"$tmp/workspace/crates/celld-forbidden-runtime/Cargo.toml" <<'MANIFEST'
[package]
name = "celld-forbidden-runtime"
version = "0.0.0"
edition = "2024"
[lib]
path = "src/lib.rs"
MANIFEST
printf '%s\n' 'pub struct ForbiddenRuntime;' >"$tmp/workspace/crates/celld-forbidden-runtime/src/lib.rs"
sed -i '/^\[lints\]/i celld-forbidden-runtime = { path = "../celld-forbidden-runtime" }' \
    "$tmp/workspace/crates/kiln-core/Cargo.toml"
printf '\npub use celld_forbidden_runtime::ForbiddenRuntime;\n' >>"$tmp/workspace/crates/kiln-core/src/lib.rs"
(
    cd "$tmp/workspace"
    cargo check -p kiln-core --lib --offline
    cargo tree -p kiln-core --edges normal --offline >"$tmp/injected.tree"
)
if check_tree kiln-core "$tmp/injected.tree" 2>"$tmp/injected.report"; then
    echo 'Celld boundary gate accepted a real forbidden core import' >&2
    exit 1
fi
if ! grep -F 'kiln-core: forbidden runtime dependency edge: ' "$tmp/injected.report" >/dev/null ||
   ! grep -F 'celld-forbidden-runtime' "$tmp/injected.report" >/dev/null; then
    cat "$tmp/injected.report" >&2
    echo 'Celld boundary gate did not identify the forbidden dependency edge' >&2
    exit 1
fi
cat "$tmp/injected.report"
