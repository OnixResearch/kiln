#!/bin/sh
# Real pinned Celld, conditional-write S3, output gating, and the Rust EventStorePort.
set -eu
umask 077

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
: "${PORT_BASE:=9960}"
case "$PORT_BASE" in ''|*[!0-9]*) echo 'PORT_BASE must be a decimal port' >&2; exit 2;; esac
[ "$PORT_BASE" -ge 1024 ] && [ "$PORT_BASE" -le 65528 ] || {
    echo 'PORT_BASE leaves no room for the seven loopback listeners' >&2; exit 2;
}
master=$PORT_BASE
volume=$((master + 1))
filer=$((master + 2))
s3=$((master + 3))
celld_port=$((master + 4))
internal=$((master + 5))
https=$((master + 6))
tmp=$(mktemp -d "${TMPDIR:-/tmp}/kiln-celld-cohort.XXXXXXXX")
seaweed_pid= caddy_pid= celld_pid=
cleanup() {
    for pid in "$caddy_pid" "$celld_pid" "$seaweed_pid"; do
        if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || :; fi
    done
    for pid in "$caddy_pid" "$celld_pid" "$seaweed_pid"; do
        if [ -n "$pid" ]; then wait "$pid" 2>/dev/null || :; fi
    done
    rm -rf -- "$tmp"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Both the release archive and tools resolve from this repository's locked flake;
# no dependency on another local checkout or manually prebuilt binary.
nix build --no-link "path:$root#celld-cohort"
source=$(nix eval --impure --raw --expr \
    'let f = builtins.getFlake (toString ./.); in toString f.packages.${builtins.currentSystem}.celld-cohort.src')
[ "$(nix hash file --type sha256 --sri "$source")" = 'sha256-DkudpH4V3EASbApSUadElsOINit+UfV+RQ+oHkOiylU=' ] || {
    echo 'Pinned Celld source hash changed' >&2; exit 1;
}
tools=$(nix build --impure --no-link --print-out-paths --expr \
    'let f = builtins.getFlake (toString ./.); p = f.inputs.nixpkgs.legacyPackages.${builtins.currentSystem}; in p.symlinkJoin { name = "kiln-celld-local-tools"; paths = [ p.rustc p.cargo p.seaweedfs p.caddy p.esbuild p.b3sum p.openssl p.curl p.gnutar ]; }')
export PATH="$tools/bin:$PATH"
cohort_digest() {
    "$tools/bin/b3sum" --derive-key "kiln.celld.$1.v1" --no-names "$2"
}
tar -xf "$source" -C "$tmp"
nix develop --offline --command sh -c '
    PATH="$1/bin:$PATH"; export PATH
    CFLAGS=-O2 CARGO_TARGET_DIR="$2"; export CFLAGS CARGO_TARGET_DIR
    exec cargo build --locked --bin celld --manifest-path "$3"
' sh "$tools" "$tmp/celld-target" "$tmp/celld-0.2.1/Cargo.toml"
celld="$tmp/celld-target/debug/celld"
[ "$("$celld" --version)" = 'celld 0.2.1 (debug)' ] || {
    echo 'Built Celld version differs from the pinned release' >&2; exit 1;
}
export CELLD_LIVE_SELECTED_ARTIFACT_BLAKE3="$(cohort_digest artifact "$celld")"
export CELLD_LIVE_SELECTED_PLATFORM="linux-$(uname -m)"

# An anonymous loopback S3 endpoint is fixture-only. Celld still authenticates
# Worker traffic and probes the backend's actual conditional-write semantics.
mkdir -p "$tmp/weed"
(
    cd "$tmp"
    exec "$tools/bin/weed" server -ip=127.0.0.1 -ip.bind=127.0.0.1 \
        -dir="$tmp/weed" -master.telemetry=false \
        -master.port="$master" -volume.port="$volume" -filer -filer.port="$filer" \
        -s3 -s3.ip.bind=127.0.0.1 -s3.port="$s3" -s3.port.iceberg=0
) >"$tmp/seaweed.log" 2>&1 &
seaweed_pid=$!
n=0
until curl --silent --show-error --fail --max-time 2 \
    "http://127.0.0.1:$s3/" >/dev/null 2>&1; do
    n=$((n + 1))
    if [ "$n" -ge 60 ]; then cat "$tmp/seaweed.log" >&2; exit 1; fi
    sleep 1
done
curl --silent --show-error --fail --max-time 5 --request PUT \
    "http://127.0.0.1:$s3/kiln-celld-test" >/dev/null

mkdir -p "$tmp/deploy/src" "$tmp/caddy-data" "$tmp/caddy-config"
nix develop --offline --command nickel export --format json config/celld-wrangler.ncl \
    >"$tmp/deploy/wrangler.jsonc"
"$tools/bin/esbuild" src/index.mjs --bundle --platform=browser --format=esm \
    --outfile="$tmp/deploy/src/index.mjs"
export CELLD_LIVE_SELECTED_WORKER_BUNDLE_BLAKE3="$(cohort_digest worker-bundle "$tmp/deploy/src/index.mjs")"
export AWS_ACCESS_KEY_ID=local-test AWS_SECRET_ACCESS_KEY=local-test-secret
export CELLD_ESBUILD="$tools/bin/esbuild"
"$celld" deploy "$tmp/deploy" --bucket s3://kiln-celld-test \
    --endpoint "http://127.0.0.1:$s3" --region us-east-1 >"$tmp/deploy.log"
cat "$tmp/deploy.log"
deployment_id=
while IFS= read -r line; do
    case "$line" in 'Current Version ID: '*) deployment_id=${line#Current Version ID: };; esac
done <"$tmp/deploy.log"
case "$deployment_id" in ''|*[!0-9a-f]*) echo 'Celld deployment version is not hexadecimal' >&2; exit 1;; esac
[ "${#deployment_id}" -eq 16 ] || { echo 'Celld deployment version is not 16 digits' >&2; exit 1; }
export CELLD_LIVE_SELECTED_DEPLOYMENT_ID="$deployment_id"
export CELLD_LIVE_SELECTED_STORAGE_COHORT="s3://kiln-celld-test@127.0.0.1:$s3"

# The proxy is loopback only. The test CA stays under tmp and is never installed
# globally; the client receives this explicit CA and validates its TLS peer.
cat >"$tmp/Caddyfile" <<EOF
{
  admin off
  skip_install_trust
  auto_https disable_redirects
}
https://127.0.0.1:$https {
  bind 127.0.0.1
  tls internal
  reverse_proxy 127.0.0.1:$celld_port {
    header_up X-Forwarded-Proto https
    header_up X-Forwarded-Host 127.0.0.1:$https
  }
}
EOF
XDG_DATA_HOME="$tmp/caddy-data" XDG_CONFIG_HOME="$tmp/caddy-config" \
    "$tools/bin/caddy" run --config "$tmp/Caddyfile" --adapter caddyfile \
    >"$tmp/caddy.log" 2>&1 &
caddy_pid=$!
root_ca="$tmp/caddy-data/caddy/pki/authorities/local/root.crt"
n=0
until [ -s "$root_ca" ]; do
    n=$((n + 1))
    if [ "$n" -ge 30 ]; then cat "$tmp/caddy.log" >&2; exit 1; fi
    sleep 1
done
export CELLD_LIVE_SELECTED_INGRESS_CA_BLAKE3="$(cohort_digest ingress-ca "$root_ca")"

secret=$(openssl rand -hex 32)
export CELLD_VAR_KILN_HMAC_SECRET="$secret" CELLD_OUTPUT_GATE=1 CELLD_STORAGE_PROBE=1
export CELLD_VAR_KILN_OUTPUT_GATE="$CELLD_OUTPUT_GATE"
export CELLD_VAR_KILN_STORAGE_PROBE="$CELLD_STORAGE_PROBE"
export CELLD_VAR_KILN_DEPLOYMENT_ID="$deployment_id"
export CELLD_WATCH="$tmp/celld-watch" CELLD_TRUST_FORWARDED_HEADERS=1
start_node() {
    (
        cd "$tmp"
        exec "$celld" --bucket s3://kiln-celld-test --endpoint "http://127.0.0.1:$s3" \
            --region us-east-1 --listen "127.0.0.1:$celld_port" \
            --internal-listen "127.0.0.1:$internal"
    ) >"$tmp/celld.log" 2>&1 &
    celld_pid=$!
    n=0
    while :; do
        response=$(curl --silent --show-error --max-time 2 --cacert "$root_ca" \
            --output /dev/null --write-out '%{http_code}' "https://127.0.0.1:$https/" 2>/dev/null) || :
        case "$response" in 400|403|409) break;; esac
        n=$((n + 1))
        if [ "$n" -ge 60 ]; then cat "$tmp/celld.log" "$tmp/caddy.log" >&2; exit 1; fi
        sleep 1
    done
    export CELLD_LIVE_OBSERVED_ARTIFACT_BLAKE3="$(cohort_digest artifact "$celld")"
    export CELLD_LIVE_OBSERVED_PLATFORM="linux-$(uname -m)"
    export CELLD_LIVE_OBSERVED_WORKER_BUNDLE_BLAKE3="$(cohort_digest worker-bundle "$tmp/deploy/src/index.mjs")"
    export CELLD_LIVE_OBSERVED_DEPLOYMENT_ID="$CELLD_VAR_KILN_DEPLOYMENT_ID"
    export CELLD_LIVE_OBSERVED_STORAGE_COHORT="s3://kiln-celld-test@127.0.0.1:$s3"
    export CELLD_LIVE_OBSERVED_INGRESS_CA_BLAKE3="$(cohort_digest ingress-ca "$root_ca")"
    export CELLD_LIVE_OBSERVED_RELEASE=v0.2.1
    export CELLD_LIVE_OBSERVED_REVISION=ae8fac053d79f971bfcb996054bb43eb2f9b05da
    export CELLD_LIVE_OBSERVED_PROTOCOL="${CELLD_VAR_KILN_WORKER_PROTOCOL_VERSION:-1}"
    export CELLD_LIVE_OBSERVED_SCHEMA="${CELLD_VAR_KILN_SCHEMA_VERSION:-1}"
    export CELLD_LIVE_OBSERVED_ORIGIN="$CELLD_LIVE_ORIGIN"
    export CELLD_LIVE_OBSERVED_OUTPUT_GATE="$CELLD_OUTPUT_GATE"
    export CELLD_LIVE_OBSERVED_STORAGE_PROBE="$CELLD_STORAGE_PROBE"
    export CELLD_LIVE_OBSERVED_AT="$(date +%s)"
}
stop_node() {
    kill "$celld_pid"
    wait "$celld_pid" 2>/dev/null || :
    celld_pid=
}
run_control() {
    CELLD_LIVE_PHASE="$1" nix develop --offline --command cargo test \
        -p kiln-adapter-celld --test live_control -- --ignored --exact \
        actual_worker_controls --nocapture
}
export CELLD_LIVE_ORIGIN="https://127.0.0.1:$https"
export CELLD_LIVE_ROOT_CA="$root_ca"
export CELLD_LIVE_SECRET="$secret"
start_node
nix develop --offline --command cargo test -p kiln-adapter-celld \
    --test live_worker -- --ignored --exact \
    actual_worker_persists_event_outbox_and_reconciles_a_lost_append_response --nocapture
run_control setup
stop_node
start_node
run_control restart
stop_node
export CELLD_VAR_KILN_SCHEMA_VERSION=2
start_node
run_control mixed
stop_node
unset CELLD_VAR_KILN_SCHEMA_VERSION
export CELLD_OUTPUT_GATE=0 CELLD_VAR_KILN_OUTPUT_GATE=0
start_node
run_control gate_disabled
stop_node
export CELLD_OUTPUT_GATE=1 CELLD_VAR_KILN_OUTPUT_GATE=1
start_node
run_control recovered
# r[verify kiln.celld.nonclaims.scenario.acknowledged]
printf 'Local Celld v0.2.1 HTTPS append/replay, lease/schema restart, mixed-schema and gate-disabled phases passed for fixture streams only.\nFramed artifact BLAKE3=%s Worker BLAKE3=%s deployment=%s storage=%s ingress CA BLAKE3=%s; gate/probe observed enabled for positive phases.\nThese observations cover only this local request, partition, transaction, deployment, runtime and storage; not remote fleets, object-store durability, provider effects, CI or release eligibility.\n' \
    "$CELLD_LIVE_SELECTED_ARTIFACT_BLAKE3" "$CELLD_LIVE_SELECTED_WORKER_BUNDLE_BLAKE3" \
    "$CELLD_LIVE_SELECTED_DEPLOYMENT_ID" "$CELLD_LIVE_SELECTED_STORAGE_COHORT" \
    "$CELLD_LIVE_SELECTED_INGRESS_CA_BLAKE3"
