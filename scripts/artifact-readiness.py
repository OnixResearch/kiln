#!/usr/bin/env python3
"""Bounded real hosted/Nix readiness rail. No builds of the harness, shared GC or VM execution."""
# r[impl kiln.readiness.benchmark] r[impl kiln.readiness.benchmark.scenario.identity]
# r[impl kiln.readiness.benchmark.scenario.controls] r[impl kiln.readiness.benchmark.scenario.phases]
import argparse
import json
import os
from pathlib import Path
import platform
import resource
import signal
import subprocess
import shutil
import sys
import time

from artifact_readiness_analysis import analyze, load_raw

MAX_COMMAND_BYTES = 4 * 1024 * 1024
COMMAND_SECONDS = 180
PHASES = {"queue_admission", "evaluation_preflight", "store_open_manifest_validation",
          "lookup_transfer_decode", "cache_save_publication", "manifest_validation", "materialization",
          "publication_collection", "setup", "execution", "publication", "restore"}


def validate(cohort):
    if set(cohort) != {"schema", "samples", "workloads", "decision"} or cohort["schema"] != "kiln.readiness.cohort.v1":
        raise ValueError("invalid cohort schema")
    def integer(value, low, high):
        return type(value) is int and low <= value <= high
    if not integer(cohort["samples"], 1, 100) or not 1 <= len(cohort["workloads"]) <= 8:
        raise ValueError("sample/workload bounds exceeded")
    seen = set()
    for work in cohort["workloads"]:
        if set(work) != {"id", "payload_bytes", "files", "symlinks", "fanout"}:
            raise ValueError("unknown workload fields")
        name = work["id"]
        if not isinstance(name, str) or not 1 <= len(name) <= 48 or not name[0].islower() or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-" for c in name) or name in seen:
            raise ValueError("invalid or duplicate workload name")
        seen.add(name)
        if not integer(work["payload_bytes"], 1, 262144) or not integer(work["files"], 1, 512) or work["payload_bytes"] % work["files"] or not integer(work["symlinks"], 0, 512) or not integer(work["fanout"], 0, 8):
            raise ValueError("invalid workload bounds or non-divisible byte control")
    decision = cohort["decision"]
    if set(decision) != {"proposal", "minimum_samples", "minimum_p95_saving_fraction", "minimum_p95_saving_ms", "maximum_retained_byte_growth"} or not integer(decision["minimum_samples"], 30, 100):
        raise ValueError("invalid decision criteria")
    if decision["minimum_p95_saving_fraction"] != .2 or decision["minimum_p95_saving_ms"] != 5 or decision["maximum_retained_byte_growth"] != 0:
        raise ValueError("predeclared decision thresholds cannot be weakened")
    return cohort


def b3(data):
    result = subprocess.run(["b3sum", "--no-names"], input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10, check=True)
    return result.stdout.decode().strip()


def child_limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_COMMAND_BYTES, MAX_COMMAND_BYTES))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


class CommandFailure(RuntimeError):
    def __init__(self, observation):
        self.observation = observation
        super().__init__(f"{observation['phase']}: {observation['exit']}; inspect {observation['stderr']}")


def command(argv, directory, label, payload=None, input_file=None, output_file=None, nix_build_directory=None):
    """Sequential children only: CPU delta excludes the external Nix daemon."""
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic_ns()
    stdout_path = output_file or directory / (label + ".stdout")
    stderr_path = directory / (label + ".stderr")
    env = {key: value for key, value in os.environ.items() if key not in {"LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH"}}
    env["NIX_CONFIG"] = "min-free = 0\nsubstitute = false\nbuilders =\nmax-jobs = 1\n"
    if nix_build_directory is not None:
        env["NIX_CONFIG"] += "build-dir = " + str(nix_build_directory) + "\n"
    env["NIX_CONF_DIR"] = str(directory)
    env["NIX_REMOTE"] = "daemon"
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        source = input_file.open("rb") if input_file else None
        try:
            process = subprocess.Popen(argv, stdin=source if source else subprocess.PIPE,
                                       stdout=stdout, stderr=stderr, env=env,
                                       start_new_session=True, preexec_fn=child_limits)
            try:
                process.communicate(payload, timeout=COMMAND_SECONDS)
                status = process.returncode
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
                status = "timeout"
            except BaseException:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.communicate()
                raise
        except OSError as error:
            status = "launch-or-io-error: " + str(error)
        finally:
            if source:
                source.close()
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    observed = {"phase": label, "elapsed_ns": time.monotonic_ns() - start,
                "user_cpu_ns": int((after.ru_utime - before.ru_utime) * 1e9),
                "system_cpu_ns": int((after.ru_stime - before.ru_stime) * 1e9),
                "cpu_scope": "sequential-command-children-excluding-nix-daemon", "injected_delay_ms": 0,
                "command": argv, "exit": status, "stdout": str(stdout_path), "stderr": str(stderr_path)}
    # Record command even if launch/output/exit fails at the provider boundary.
    with (directory / "commands.jsonl").open("a") as stream:
        stream.write(json.dumps(observed) + "\n")
    if status != 0:
        raise CommandFailure(observed)
    return stdout_path.read_bytes(), observed


def nix_expression(work, builder, coreutils, nonce):
    quote = json.dumps
    # All shell code is repository-owned; only validated integer workload values enter it.
    unit = work["payload_bytes"] // work["files"]
    script = f'''set -eu
mkdir -p "$out"
i=0
while [ "$i" -lt {work['files']} ]; do
  name=$(printf '%04d' "$i")
  head -c {unit} /dev/zero | tr '\\000' x > "$out/f$name"
  i=$((i+1))
done
i=0
while [ "$i" -lt {work['symlinks']} ]; do
  ln -s f0000 "$out/link$i"
  i=$((i+1))
done
'''
    for i in range(work["fanout"]):
        script += f'ln -s "$dep{i}" "$out/dep{i}"\n'
    deps = " ".join(f"dep{i} = branch {i};" for i in range(work["fanout"]))
    return f'''let
  builder = builtins.storePath {quote(str(builder))};
  tools = builtins.storePath {quote(str(coreutils))};
  make = name: script: extra: builtins.derivation ({{
    inherit name builder;
    system = builtins.currentSystem;
    args = [ "-c" script ];
    PATH = "${{tools}}/bin";
    cohortNonce = {quote(nonce)};
    preferLocalBuild = true;
    allowSubstitutes = false;
  }} // extra);
  shared = make "kiln-readiness-shared" "mkdir -p $out; printf shared > $out/content" {{}};
  branch = i: make ("kiln-readiness-branch-" + builtins.toString i)
    "mkdir -p $out; ln -s $shared $out/shared" {{ inherit shared; }};
in make "kiln-readiness-{work['id']}" {quote(script)} {{ {deps} }}
'''


def inspect_product(root, work):
    entries = []
    content = bytearray()
    files = symlinks = 0
    shared_targets = set()
    expected_files = {f"f{i:04}" for i in range(work["files"])}
    expected_links = {f"link{i}" for i in range(work["symlinks"])}
    expected_dependencies = {f"dep{i}" for i in range(work["fanout"])}
    for path in sorted(root.iterdir()):
        if path.is_symlink():
            symlinks += 1
            target = os.readlink(path)
            if path.name in expected_links:
                if target != "f0000":
                    raise ValueError("symlink semantic target mismatch")
            elif path.name in expected_dependencies:
                if not target.startswith("/nix/store/"):
                    raise ValueError("dependency reference is not an exact store path")
                shared = Path(target) / "shared"
                if not shared.is_symlink():
                    raise ValueError("fanout branch has no shared dependency")
                shared_target = os.readlink(shared)
                if not shared_target.startswith("/nix/store/") or (Path(shared_target) / "content").read_bytes() != b"shared":
                    raise ValueError("shared dependency content mismatch")
                shared_targets.add(shared_target)
            else:
                raise ValueError("unexpected symlink entry")
            entries.append([path.name, "symlink", target])
        elif path.is_file():
            body = path.read_bytes()
            files += 1
            if path.name not in expected_files or body != b"x" * (work["payload_bytes"] // work["files"]):
                raise ValueError("unexpected program payload")
            content.extend(body)
            entries.append([path.name, "file", len(body)])
        else:
            raise ValueError("unexpected materialized entry")
    if files != work["files"] or symlinks != work["symlinks"] + work["fanout"] or len(content) != work["payload_bytes"]:
        raise ValueError("materialized count/byte mismatch")
    if work["fanout"] and len(shared_targets) != 1:
        raise ValueError("fanout branches do not share exactly one dependency")
    payload_identity = b3(content)
    # Normalize native store names only after verifying the actual graph/content;
    # retain the unnormalized exact manifest alongside this same-work identity.
    semantics = {"payload_blake3": payload_identity, "files": files, "symlinks": symlinks,
                 "fanout": work["fanout"], "shared_content": "shared" if shared_targets else None}
    return {"manifest_blake3": b3(json.dumps(entries, sort_keys=True).encode()),
            "semantic_blake3": b3(json.dumps(semantics, sort_keys=True).encode()),
            "payload_blake3": payload_identity, "regular_files": files, "symlinks": symlinks,
            "payload_bytes": len(content), "direct_dependency_references": work["fanout"],
            "shared_dependency_paths": sorted(shared_targets)}


def nix_samples(args, work, directory, emit):
    for index in range(args.samples):
        sample = directory / str(index)
        sample.mkdir(mode=0o700)
        expression = sample / "workload.nix"
        expression.write_text(nix_expression(work, args.nix_builder, args.nix_coreutils, str(sample)))
        drv = None
        output = None
        for state in ("cold-local-miss", "warm-local-hit"):
            attempt = sample / state
            attempt.mkdir(mode=0o700)
            observations = []
            row = {"provider": "nix-local", "profile": "nix-store-readiness-v1", "workload": work,
                   "index": index, "cache_state": state, "status": "incomplete", "observations": observations,
                   "delay_phase": None, "delay_ms": 0,
                   "unavailable": ["remote-object-tier", "queue-admission", "guest-hydration", "boot", "download", "decompression"],
                   "coalesced": ["realization_execution_import_publication"], "root": str(attempt)}
            def call(name, argv, **kwargs):
                data, observation = command(argv, attempt, name, nix_build_directory=args.nix_build_directory, **kwargs)
                observations.append(observation)
                return data
            try:
                drv = call("evaluation_preflight", ["nix-instantiate", str(expression)]).decode().strip()
                if not drv.startswith("/nix/store/") or "\n" in drv:
                    raise ValueError("invalid exact derivation identity")
                row["derivation"] = drv
                call("planning", ["nix-store", "--realise", "--dry-run", drv])
                output_root = attempt / "output-root"
                reported_root = call("realization_execution_import_publication", ["nix-store", "--add-root", str(output_root), "--indirect", "--realise", drv]).decode().strip()
                if reported_root != str(output_root) or not output_root.is_symlink():
                    raise ValueError("realization did not publish its exact owned GC root")
                output = str(output_root.resolve(strict=True))
                row["realization_root"] = reported_root
                if not output.startswith("/nix/store/") or "\n" in output:
                    raise ValueError("invalid output identity")
                row["output_path"] = output
                row["native_path_info"] = json.loads(call("metadata_lookup", ["nix", "--extra-experimental-features", "nix-command", "path-info", "--json", output]))
                archive = attempt / "output.nar"
                call("publication_nar_export", ["nix-store", "--dump", output], output_file=archive)
                row["archive_bytes"] = archive.stat().st_size
                row["archive_blake3"] = b3(archive.read_bytes())
                restored = attempt / "materialized"
                call("materialization", ["nix-store", "--restore", str(restored)], input_file=archive)
                before = resource.getrusage(resource.RUSAGE_SELF)
                start = time.monotonic_ns()
                facts = inspect_product(restored, work)
                after = resource.getrusage(resource.RUSAGE_SELF)
                observations.append({"phase": "execution", "elapsed_ns": time.monotonic_ns() - start,
                                     "user_cpu_ns": int((after.ru_utime - before.ru_utime) * 1e9),
                                     "system_cpu_ns": int((after.ru_stime - before.ru_stime) * 1e9),
                                     "cpu_scope": "analysis-process-excluding-b3sum-children", "injected_delay_ms": 0})
                row["facts"] = facts
                row["output_identity"] = facts["semantic_blake3"]
                row["status"] = "complete"
            except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                if isinstance(error, CommandFailure):
                    observations.append(error.observation)
                row["status"] = "failed"
                row["error"] = str(error)
            emit(row)
            if row["status"] != "complete":
                break


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new owned directory; artifacts are retained")
    parser.add_argument("--hosted-binary", type=Path, help="already built artifact-readiness example")
    parser.add_argument("--nix-builder", type=Path, help="existing immutable /nix/store/.../bin/bash")
    parser.add_argument("--nix-coreutils", type=Path, help="existing immutable coreutils store root")
    parser.add_argument("--nix-build-directory", type=Path, help="existing sandbox-accessible native build parent, separate from private retained evidence")
    parser.add_argument("--samples", type=int, help="bounded override, 1..100")
    parser.add_argument("--workload", action="append", help="select named workload(s)")
    parser.add_argument("--delay-phase", choices=sorted(PHASES))
    parser.add_argument("--delay-ms", type=int, default=0)
    args = parser.parse_args()
    args.cohort = Path("benchmarks/artifact-readiness/cohort.ncl")
    exported = subprocess.run(["nickel", "export", "--format", "json", str(args.cohort)], check=True, stdout=subprocess.PIPE, timeout=30).stdout
    if len(exported) > 65536:
        parser.error("cohort exceeds bound")
    cohort = validate(json.loads(exported))
    args.samples = cohort["samples"] if args.samples is None else args.samples
    if not 1 <= args.samples <= 100 or not 0 <= args.delay_ms <= 1000 or (args.delay_ms and not args.delay_phase):
        parser.error("invalid sample/delay bounds")
    if not args.hosted_binary and not args.nix_builder:
        parser.error("select an actual provider: --hosted-binary or --nix-builder with --nix-coreutils")
    if bool(args.nix_builder) != bool(args.nix_coreutils):
        parser.error("Nix requires both immutable builder and coreutils paths")
    if args.nix_builder:
        if args.nix_build_directory is None:
            parser.error("Nix requires --nix-build-directory accessible to its sandbox builder")
        args.nix_build_directory = args.nix_build_directory.resolve(strict=True)
        if not args.nix_build_directory.is_dir() or any(c in str(args.nix_build_directory) for c in "\r\n"):
            parser.error("Nix build directory must be an existing directory without control separators")
    for path in (args.nix_builder, args.nix_coreutils):
        if path and (not str(path).startswith("/nix/store/") or not path.exists()):
            parser.error("Nix cohort tools must already exist under /nix/store")
    if args.delay_phase and not args.hosted_binary:
        parser.error("phase injection is supported only by the opt-in hosted observation scope")
    selected = [work for work in cohort["workloads"] if not args.workload or work["id"] in args.workload]
    if not selected or (args.workload and set(args.workload) != {work["id"] for work in selected}):
        parser.error("unknown or empty workload selection")
    args.output = args.output.absolute()
    args.output.mkdir(mode=0o700)
    (args.output / "cohort.json").write_bytes(exported)
    (args.output / "cohort.ncl").write_bytes(args.cohort.read_bytes())
    (args.output / "contract.ncl").write_bytes((args.cohort.parent / "contract.ncl").read_bytes())
    cpu = Path("/proc/cpuinfo").read_text()
    runner = {"hostname": platform.node(), "machine": platform.machine(), "kernel": platform.release(),
              "cpu_identity": sorted(set(line for line in cpu.splitlines() if line.split(":", 1)[0].strip() in
                  {"vendor_id", "cpu family", "model", "model name", "stepping", "microcode", "flags", "cache size", "address sizes"})),
              "filesystem_device": args.output.stat().st_dev,
              "cpu_affinity": sorted(os.sched_getaffinity(0)), "kernel_page_cache": "uncontrolled; no drop-caches"}
    source_files = [Path(__file__), Path("scripts/artifact_readiness_analysis.py"), args.cohort,
                    Path("benchmarks/artifact-readiness/contract.ncl"), Path("crates/kiln-hosted/src/cache.rs"),
                    Path("crates/kiln-hosted/src/cache/readiness.rs"), Path("crates/kiln-hosted/examples/artifact-readiness.rs")]
    identities = {str(path): b3(path.read_bytes()) for path in source_files}
    if args.hosted_binary:
        args.hosted_binary = args.hosted_binary.absolute()
        identities[str(args.hosted_binary)] = b3(args.hosted_binary.read_bytes())
    tools = {}
    for name in ["nickel", "b3sum", "jj"] + (["nix-instantiate", "nix-store", "nix"] if args.nix_builder else []):
        executable = shutil.which(name)
        if executable is None:
            raise ValueError("required native tool unavailable: " + name)
        executable = Path(executable).resolve()
        version = subprocess.run([str(executable), "--version"], check=True, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, timeout=10).stdout.decode().strip()
        tools[name] = {"path": str(executable), "blake3": b3(executable.read_bytes()), "version": version}
    revision = subprocess.run(["jj", "log", "-r", "@", "--no-graph", "-T", "commit_id"], check=True, stdout=subprocess.PIPE, timeout=30).stdout.decode()
    metadata = {"schema": "kiln.readiness.run.v1", "revision": revision, "source_files": identities,
                "tools": tools,
                "runner": runner, "runner_identity": b3(json.dumps(runner, sort_keys=True).encode()),
                "command": sys.argv, "samples_override": args.samples,
                "providers": {"hosted": str(args.hosted_binary) if args.hosted_binary else None,
                              "nix_builder": str(args.nix_builder) if args.nix_builder else None,
                              "nix_coreutils": str(args.nix_coreutils) if args.nix_coreutils else None,
                              "nix_build_directory": str(args.nix_build_directory) if args.nix_build_directory else None},
                "decision_predeclared": cohort["decision"], "remote_object_tier": "absent: no admitted provider configured"}
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2))
    (args.output / "cpuinfo.txt").write_text(cpu)
    # Native mount identity is retained, not inferred from a local disk label.
    (args.output / "mountinfo.txt").write_text(Path("/proc/self/mountinfo").read_text())
    raw = args.output / "samples.jsonl"
    with raw.open("x") as stream:
        def emit(row):
            row.update(schema="kiln.readiness.sample.v1", runner_identity=metadata["runner_identity"],
                       source_identity=b3(json.dumps(row["workload"], sort_keys=True).encode()))
            profile = {"label": row["profile"]}
            if row["provider"] == "nix-local":
                profile.update(builder=str(args.nix_builder), coreutils=str(args.nix_coreutils),
                               build_directory=str(args.nix_build_directory),
                               tools={name: tools[name] for name in ("nix", "nix-instantiate", "nix-store")})
            row["profile_identity"] = b3(json.dumps(profile, sort_keys=True).encode())
            stream.write(json.dumps(row) + "\n")
            stream.flush()
        for work in selected:
            if args.hosted_binary:
                directory = args.output / (work["id"] + "-hosted")
                directory.mkdir(mode=0o700)
                if work["symlinks"] or work["fanout"]:
                    emit({"provider": "hosted-local", "profile": "readiness-cache-v1", "workload": work,
                          "index": 0, "cache_state": "unsupported", "status": "absent", "observations": [],
                          "delay_phase": args.delay_phase, "delay_ms": args.delay_ms,
                          "error": "hosted cache deliberately denies symlinks/reference entries; use Nix cohort"})
                else:
                    request = {"root": str(directory / "owned"), "workload": work, "samples": args.samples,
                               "delay_phase": args.delay_phase, "delay_ms": args.delay_ms}
                    try:
                        data, _ = command([str(args.hosted_binary)], directory, "hosted_cohort",
                                          payload=json.dumps(request).encode())
                        for line in data.splitlines():
                            emit(json.loads(line))
                    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                        # Preserve partially emitted attempts on failure; do not replace them with success.
                        partial = directory / "hosted_cohort.stdout"
                        if partial.exists():
                            for line in partial.read_bytes().splitlines():
                                try:
                                    emit(json.loads(line))
                                except json.JSONDecodeError:
                                    pass
                        emit({"provider": "hosted-local", "profile": "readiness-cache-v1", "workload": work,
                              "index": -1, "cache_state": "cohort-incomplete", "status": "incomplete", "observations": [],
                              "delay_phase": args.delay_phase, "delay_ms": args.delay_ms, "error": str(error)})
            if args.nix_builder:
                directory = args.output / (work["id"] + "-nix")
                directory.mkdir(mode=0o700)
                nix_samples(args, work, directory, emit)
    report = analyze(load_raw(raw))
    (args.output / "analysis.json").write_text(json.dumps(report, indent=2))
    print(args.output / "analysis.json")
    if any(cell["statuses"].get("failed", 0) or cell["statuses"].get("incomplete", 0) or cell["attribution_errors"] for cell in report["cells"]):
        return 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        sys.exit(f"artifact readiness: {error}")
