#!/usr/bin/env bash
# Composed Lattice #6 / Kiln #45 qualification over Kiln's real
# `kiln-lattice-dispatch --worker-request` transport and the real `lattice`
# binary. Every worker is the pinned OMP in bubblewrap, reaching only a local
# fault endpoint (endpoint.py); no external model is contacted.
#
# Required environment: LATTICE (immutable store path of bin/lattice), KILN
# (kiln-lattice-dispatch), HARNESS (k45-harness), OMP, BWRAP, SOCAT, WORKER_SHELL,
# OUT (fresh output root). Usage: qualify.sh SCENARIO...
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
: "${LATTICE:?}" "${KILN:?}" "${HARNESS:?}" "${OMP:?}" "${BWRAP:?}" "${SOCAT:?}" "${WORKER_SHELL:?}" "${OUT:?}"
b3() { b3sum --no-names "$1"; }
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd -P)

# One fresh Kiln store, claimed attempt, endpoint, prepared request and bound plan.
attempt() {
  local name=$1 mode=$2 dir="$OUT/$1"
  mkdir -p "$dir/source/src" "$dir/workspace/src"
  chmod 700 "$dir"
  printf 'pub fn answer() -> i32 { 41 }\n' > "$dir/source/src/main.rs"
  git -C "$dir/source" init -q
  git -C "$dir/source" -c user.name=qualification -c user.email=q@invalid add src/main.rs
  git -C "$dir/source" -c user.name=qualification -c user.email=q@invalid commit -qm base
  git -C "$dir/source" archive HEAD | tar -x -C "$dir/workspace"
  chmod 644 "$dir/workspace/src/main.rs"
  local endpoint model=qualification-local
  if [ "$mode" = live ]; then
    # Operator-approved authless loopback model gateway; never run by this slice.
    endpoint=${LIVE_ENDPOINT:?} model=${LIVE_MODEL:?}
    : > "$dir/endpoint.log"; echo 0 > "$dir/endpoint.pid"
  else
    python3 "$here/endpoint.py" "$mode" "$dir/port" "$dir/endpoint.log" "$dir/release" &
    echo $! > "$dir/endpoint.pid"
    for _ in $(seq 100); do [ -s "$dir/port" ] && break; sleep 0.1; done
    : > "$dir/endpoint.log"
    endpoint="http://127.0.0.1:$(cat "$dir/port")/v1/chat/completions"
  fi
  "$HARNESS" setup --database "$dir/kiln.sqlite" --enrollment "$dir/enrollment.json" \
    --base "$(git -C "$dir/source" rev-parse HEAD)" --source-id "lattice-6/task:$name" \
    --workspace "$dir/workspace" --omp "$OMP" --omp-blake3 "$(b3 "$OMP")" \
    --bwrap "$BWRAP" --bwrap-blake3 "$(b3 "$BWRAP")" --socat "$SOCAT" --socat-blake3 "$(b3 "$SOCAT")" \
    --shell "$WORKER_SHELL" --shell-blake3 "$(b3 "$WORKER_SHELL")" \
    --endpoint "$endpoint" --model "$model" \
    --seed "$dir/seed.json" > "$dir/setup.json"
  # Kiln #45 follow-up: the request binds Kiln's dispatch plan. Prepare once for
  # the worker profile, bind the plan, add its identity and prepare again.
  "$LATTICE" implementation-attempt prepare --request "$dir/seed.json" --output "$dir/request-probe.json" > "$dir/prepare-probe.json"
  "$HARNESS" bind --database "$dir/kiln.sqlite" --enrollment "$dir/enrollment.json" \
    --request "$dir/request-probe.json" --plan "$dir/plan.json" --observations "$dir/observations.json" > "$dir/bind.json"
  jq --arg plan "$(jq -r .plan "$dir/bind.json")" '.dispatch_plan = $plan' "$dir/seed.json" > "$dir/seed-planned.json"
  "$LATTICE" implementation-attempt prepare --request "$dir/seed-planned.json" --output "$dir/request.json" > "$dir/prepare.json"
  [ "$(jq -r .worker_profile "$dir/prepare.json")" = "$(jq -r .worker_profile "$dir/prepare-probe.json")" ]
  printf '{"schema":"kiln.lattice-implementation-cli.v2","lattice_program":"%s","executable_blake3":"%s","state_directory":"%s","output_directory":"%s"}\n' \
    "$LATTICE" "$(b3 "$LATTICE")" "$dir/state" "$dir/transport" > "$dir/transport.json"
}

# Kiln's real worker transport; records exit code, stdout and stderr per call.
dispatch() {
  local dir="$OUT/$1" label=$2 plan=${3:-$OUT/$1/plan.json} request=${4:-$OUT/$1/request.json}
  set +e
  "$KILN" --database "$dir/kiln.sqlite" --enrollment "$dir/enrollment.json" --dispatch-plan "$plan" \
    --worker-request "$request" --worker-transport-profile "$dir/transport.json" \
    --admission-observations "$dir/observations.json" > "$dir/$label.stdout" 2> "$dir/$label.stderr"
  echo $? > "$dir/$label.exit"
  set -e
}

finish() {
  local dir="$OUT/$1"
  [ "$(cat "$dir/endpoint.pid")" = 0 ] || kill "$(cat "$dir/endpoint.pid")" 2>/dev/null || true
  wc -l < "$dir/endpoint.log" > "$dir/endpoint.count"
  "$HARNESS" status --database "$dir/kiln.sqlite" --attempt "$(jq -r .attempt "$dir/setup.json")" > "$dir/kiln-attempt.json" 2>&1 || true
  find "$dir/state" -type f -printf '%P %s\n' 2>/dev/null | sort > "$dir/state.files" || true
}

wait_for_request() {
  for _ in $(seq 3000); do [ -s "$OUT/$1/endpoint.log" ] && return 0; sleep 0.1; done
  return 1
}

# A: one normalized terminal observation that Kiln accepts, then an exact
# duplicate dispatch that reconciles instead of executing again.
scenario_run() {
  attempt run fail
  dispatch run dispatch
  dispatch run duplicate
  # Substituted request bytes under the same attempt: Kiln refuses them before
  # transport; Lattice refuses them against the retained intent.
  jq '.omp.prompt = "Substituted task under the same attempt."' "$OUT/run/request.json" > "$OUT/run/request-substituted.json"
  dispatch run kiln-substituted "$OUT/run/plan.json" "$OUT/run/request-substituted.json"
  set +e
  "$LATTICE" implementation-attempt run --request "$OUT/run/request-substituted.json" --state "$OUT/run/state" \
    > "$OUT/run/lattice-substituted.stdout" 2> "$OUT/run/lattice-substituted.stderr"
  echo $? > "$OUT/run/lattice-substituted.exit"
  "$LATTICE" implementation-attempt run --request "$OUT/run/request.json" --state "$OUT/run/state" \
    > "$OUT/run/lattice-duplicate.stdout" 2> "$OUT/run/lattice-duplicate.stderr"
  echo $? > "$OUT/run/lattice-duplicate.exit"
  set -e
  finish run
}

# B: Kiln persisted transport intent and was lost before any Lattice process
# started; re-entry recovers by closing the attempt, twice.
scenario_lost_before_start() {
  attempt lost-before-start fail
  local dir="$OUT/lost-before-start"
  # Kiln #45 review: a first launch binds its program and state root (and
  # creates that root) before the transport intent, exactly as worker::run does.
  mkdir -p -m 700 "$dir/transport" "$dir/state"
  jq -cn --arg attempt "$(jq -r .attempt "$dir/setup.json")" --argjson fence "$(jq -r .fence "$dir/setup.json")" \
    --arg program "$LATTICE" --arg blake3 "$(b3 "$LATTICE")" --arg state "$dir/state" \
    '{schema:"kiln.lattice-transport-binding.v1",attempt:$attempt,fence:$fence,lattice_program:$program,executable_blake3:$blake3,state_directory:$state}' \
    > "$dir/transport/transport-binding-$(jq -r .attempt "$dir/setup.json")-$(jq -r .fence "$dir/setup.json").json"
  "$HARNESS" begin-transport --database "$dir/kiln.sqlite" --enrollment "$dir/enrollment.json" \
    --request "$dir/request.json" --plan "$dir/plan.json" \
    --observations "$dir/observations.json" > "$dir/begin-transport.json"
  dispatch lost-before-start recover
  dispatch lost-before-start recover-replay
  finish lost-before-start
}

# I (Kiln #45 review): the first run executes under root A; the profile then
# names root B. Kiln's second call recovers with the bound root A, reconciles
# the retained execution, and never invokes Lattice in B.
scenario_moved_root() {
  local dir="$OUT/moved-root"
  attempt moved-root fail
  dispatch moved-root dispatch
  mkdir -p "$dir/state-b"
  jq --arg root "$dir/state-b" '.state_directory = $root' "$dir/transport.json" > "$dir/transport-b.json"
  cp "$dir/transport-b.json" "$dir/transport.json"
  dispatch moved-root recover-moved
  find "$dir/state-b" -mindepth 1 | wc -l > "$dir/state-b.count"
  finish moved-root
}

# C: the controller is killed while Lattice runs; the worker finishes; re-entry
# recovers the retained result without a second execution.
scenario_lost_during_run() {
  local dir="$OUT/lost-during-run"
  attempt lost-during-run hold
  ( dispatch lost-during-run dispatch ) &
  local runner=$!
  wait_for_request lost-during-run
  pkill -KILL -f "kiln-lattice-dispatch --database $dir/kiln.sqlite" || true
  wait "$runner" || true
  touch "$dir/release"
  for _ in $(seq 3000); do
    [ -n "$(find "$dir/state" -name omp-observation.json 2>/dev/null)" ] && ! pgrep -f "implementation-attempt run --request $dir/transport" >/dev/null && break
    sleep 0.1
  done
  dispatch lost-during-run recover
  dispatch lost-during-run recover-replay
  finish lost-during-run
}

# D: crossed plan fence at Kiln; higher fence, crossed attempt and stale fence at Lattice.
scenario_refusals() {
  local dir="$OUT/refusals"
  attempt refusals fail
  # Kiln-level: a plan whose fence differs from the request.
  jq '.fence += 1' "$dir/plan.json" > "$dir/plan-crossed.json"
  dispatch refusals kiln-crossed-fence "$dir/plan-crossed.json"
  # Kiln #45 follow-up: a request without the reserved dispatch plan is refused
  # before transport intent.
  dispatch refusals kiln-unplanned "$dir/plan.json" "$dir/request-probe.json"
  # Lattice-level: a higher fence of the same work, then the current attempt.
  local higher="$dir/seed-higher.json"
  jq '.envelope.fence += 1 | .envelope.attempt_id = ("f" * 64) |
      .worker_compute = {"kind":"provider_execution"} |
      .execution_binding.compute = {"kind":"provider_allocation","allocation_id":"alloc","host_id":"host","consumer_id":"consumer","owner_generation":1}' \
    "$dir/seed.json" > "$higher"
  "$LATTICE" implementation-attempt prepare --request "$higher" --output "$dir/request-higher.json" > "$dir/prepare-higher.json"
  set +e
  "$LATTICE" implementation-attempt run --request "$dir/request-higher.json" --state "$dir/state" > "$dir/lattice-higher.stdout" 2> "$dir/lattice-higher.stderr"
  echo $? > "$dir/lattice-higher.exit"
  jq '.envelope.attempt_id = ("e" * 64)' "$higher" > "$dir/seed-crossed.json"
  "$LATTICE" implementation-attempt prepare --request "$dir/seed-crossed.json" --output "$dir/request-crossed.json" > /dev/null
  "$LATTICE" implementation-attempt run --request "$dir/request-crossed.json" --state "$dir/state" > "$dir/lattice-crossed.stdout" 2> "$dir/lattice-crossed.stderr"
  echo $? > "$dir/lattice-crossed.exit"
  set -e
  # Kiln's current attempt at the lower fence now reaches Lattice and is refused
  # with dispatched:false; Kiln settles it before any effect. A replay finds the
  # attempt settled and never reaches Lattice again.
  dispatch refusals stale-fence
  dispatch refusals stale-fence-replay
  finish refusals
}

# G (Kiln #45 follow-up): Lattice records another attempt at the same work and
# fence without starting a worker; Kiln's attempt is refused as crossed_attempt
# with dispatched:false and settled.
scenario_crossed() {
  local dir="$OUT/crossed"
  attempt crossed fail
  jq '.envelope.attempt_id = ("e" * 64) |
      .worker_compute = {"kind":"provider_execution"} |
      .execution_binding.compute = {"kind":"provider_allocation","allocation_id":"alloc","host_id":"host","consumer_id":"consumer","owner_generation":1}' \
    "$dir/seed.json" > "$dir/seed-other.json"
  "$LATTICE" implementation-attempt prepare --request "$dir/seed-other.json" --output "$dir/request-other.json" > /dev/null
  set +e
  "$LATTICE" implementation-attempt run --request "$dir/request-other.json" --state "$dir/state" > "$dir/lattice-other.stdout" 2> "$dir/lattice-other.stderr"
  echo $? > "$dir/lattice-other.exit"
  set -e
  dispatch crossed crossed-attempt
  dispatch crossed crossed-replay
  finish crossed
}

# H (Kiln #45 follow-up): the attempt directory already holds the executed
# intent of other request bytes. Lattice refuses Kiln's request as
# substituted_request with dispatched:true; Kiln must retain the attempt for
# reconciliation on dispatch and on recovery.
scenario_substituted_dispatched() {
  local dir="$OUT/substituted-dispatched"
  attempt substituted-dispatched fail
  jq '.omp.prompt = "Substituted task under the same attempt."' "$dir/seed-planned.json" > "$dir/seed-substituted.json"
  "$LATTICE" implementation-attempt prepare --request "$dir/seed-substituted.json" --output "$dir/request-substituted.json" > /dev/null
  set +e
  "$LATTICE" implementation-attempt run --request "$dir/request-substituted.json" --state "$dir/state" > "$dir/lattice-substituted.stdout" 2> "$dir/lattice-substituted.stderr"
  echo $? > "$dir/lattice-substituted.exit"
  set -e
  dispatch substituted-dispatched dispatch
  dispatch substituted-dispatched recover
  finish substituted-dispatched
}

# E: SIGTERM to Kiln after the worker reached the endpoint; Kiln forwards
# `implementation-attempt cancel`, Lattice cancels the worker process group.
scenario_cancel() {
  local dir="$OUT/cancel"
  attempt cancel hold
  ( dispatch cancel dispatch ) &
  local runner=$!
  wait_for_request cancel
  pkill -TERM -f "kiln-lattice-dispatch --database $dir/kiln.sqlite" || true
  wait "$runner" || true
  finish cancel
}

# F (parent only, live model): one model-produced candidate through Kiln.
scenario_live() {
  attempt live live
  dispatch live dispatch
  finish live
}

for scenario in "$@"; do "scenario_$scenario"; done
