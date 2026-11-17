#!/usr/bin/env python3
"""Opt-in, Linux cgroup-v2 Rust parallel frontend contention experiment."""
# r[impl kiln.build.parallel_frontend_evaluation]
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import uuid

from parallel_frontend_analysis import analyze

DEFAULT_THREADS = "1,2,4,8"
DEFAULT_CONCURRENCY = "1,2,4"
STDERR_TAIL_BYTES = 8192
RSS_SAMPLE_SECONDS = 0.05
MAX_CARGO_JOBS = 1024
# Every Cargo invocation compiles with rustc itself: no configured compiler
# cache wrapper, no incremental state and an explicit job count. A cache hit or
# the operator's default `build.jobs` would otherwise change what is measured.
CONTROLLED_CARGO_ENVIRONMENT = {"RUSTC_WRAPPER": "", "RUSTC_WORKSPACE_WRAPPER": "", "CARGO_INCREMENTAL": "0"}
SCOPE_WRAPPER = r'''
start=$1; end=$2; stats=$3; shift 3
path=
while IFS=: read -r hierarchy controllers component; do
    if [ "$hierarchy" = 0 ]; then path=$component; break; fi
done < /proc/self/cgroup
if [ -n "$path" ]; then printf '%s\n' "$path" > "$stats/cgroup.path"; fi
printf '%s\n' "$(date +%s%N)" > "$start"
"$@"
status=$?
printf '%s\n' "$(date +%s%N)" > "$end"
if [ -n "$path" ]; then
    cp "/sys/fs/cgroup${path}/cpu.stat" "$stats/cpu.stat" 2>/dev/null || :
    cp "/sys/fs/cgroup${path}/memory.peak" "$stats/memory.peak" 2>/dev/null || :
    cp "/sys/fs/cgroup${path}/memory.events" "$stats/memory.events" 2>/dev/null || :
fi
exit "$status"
'''


def csv_ints(value, name, allowed):
    try:
        entries = [int(part) for part in value.split(",")]
    except ValueError as error:
        raise ValueError(f"invalid {name} list") from error
    if not entries or len(set(entries)) != len(entries) or not set(entries) <= allowed:
        raise ValueError(f"invalid {name} list")
    return entries


def validate(args):
    if not re.fullmatch(r"nightly-\d{4}-\d{2}-\d{2}(?:-[\w-]+)?", args.toolchain):
        raise ValueError("--toolchain requires an exact dated nightly (not nightly/stable/beta)")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[\w-]+)?", args.baseline_toolchain):
        raise ValueError("--baseline-toolchain must name an exact pinned version")
    if args.baseline_toolchain == args.toolchain:
        raise ValueError("baseline and nightly must differ")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", args.crate):
        raise ValueError("invalid crate name")
    args.threads = csv_ints(args.threads, "threads", {1, 2, 4, 8})
    args.concurrency = csv_ints(args.concurrency, "concurrency", {1, 2, 4, 8})
    if 1 not in args.threads or 1 not in args.concurrency or len(args.concurrency) < 2:
        raise ValueError("threads and concurrency must include 1; choose at least two concurrency levels")
    if not 1 <= args.repeats <= 100 or (args.mode == "build" and args.repeats < 2):
        raise ValueError("repeats must be 1..100, at least 2 for build hashes")
    if not re.fullmatch(r"[1-9]\d*(?:\.\d+)?%", args.cpu_quota):
        raise ValueError("--cpu-quota must be a positive systemd percentage (e.g. 200%)")
    if not re.fullmatch(r"[1-9]\d*[KMGT]?", args.memory_max):
        raise ValueError("--memory-max must be a positive systemd size (e.g. 4G)")
    if args.cargo_jobs is None:
        args.cargo_jobs = os.cpu_count()
    if not isinstance(args.cargo_jobs, int) or not 1 <= args.cargo_jobs <= MAX_CARGO_JOBS:
        raise ValueError(f"--cargo-jobs must be 1..{MAX_CARGO_JOBS}")
    if not args.source_revision or any(char in args.source_revision for char in "\r\n"):
        raise ValueError("--source-revision must identify the exact checked-out revision")
    if args.out.exists() or args.out.is_symlink():
        raise ValueError("--out must be a new directory")
    if args.mode == "build" and shutil.which("b3sum") is None:
        raise ValueError("build artifact hash mode requires b3sum")
    for tool in ("rustup", "cp", "systemd-run"):
        if shutil.which(tool) is None:
            raise ValueError(f"missing required tool: {tool}")
    return args


def host_memory_bytes():
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemTotal:"):
            return int(line.split()[1]) * 1024
    raise ValueError("host MemTotal unavailable")


def rustup(toolchain, *command):
    return ["rustup", "run", toolchain, *command]


def cargo_environment(jobs):
    """The inherited environment with the controlled Cargo settings applied."""
    environment = dict(os.environ)
    environment.update(CONTROLLED_CARGO_ENVIRONMENT)
    environment["CARGO_BUILD_JOBS"] = str(jobs)
    return environment


def run_checked(argv, environment=None):
    subprocess.run(argv, check=True, env=environment)


def counter(path, key):
    try:
        for line in path.read_text().splitlines():
            name, value = line.split()
            if name == key:
                return int(value)
    except (FileNotFoundError, ValueError):
        return None
    return None


def scalar(path):
    try:
        return int(path.read_text().strip())
    except (FileNotFoundError, ValueError):
        return None


def sampled_scope_rss(directory):
    """Sum resident pages of processes in this scope; a sampled lower bound."""
    try:
        component = (directory / "cgroup.path").read_text().strip()
        if not component.startswith("/") or ".." in Path(component).parts:
            return None
        pids = (Path("/sys/fs/cgroup") / component.lstrip("/") / "cgroup.procs").read_text().splitlines()
    except (FileNotFoundError, PermissionError):
        return None
    pages = 0
    observed = False
    for pid in pids:
        if not pid.isdecimal():
            continue
        try:
            resident = int((Path("/proc") / pid / "statm").read_text().split()[1])
        except (FileNotFoundError, ProcessLookupError, PermissionError, IndexError, ValueError):
            continue
        pages += resident
        observed = True
    return pages * os.sysconf("SC_PAGE_SIZE") if observed else None

def stderr_tail(path):
    with path.open("rb") as stream:
        stream.seek(0, os.SEEK_END)
        stream.seek(max(stream.tell() - STDERR_TAIL_BYTES, 0))
        return stream.read().decode("utf-8", "replace")

def compiler_commands(path):
    """Cargo -vv emits the full rustc invocation as a Running line."""
    with path.open(encoding="utf-8", errors="replace") as stream:
        return [line.strip() for line in stream if line.lstrip().startswith("Running `")]


def artifact_path(stdout_path, crate):
    target_name = crate.replace("-", "_")
    found = []
    with stdout_path.open(encoding="utf-8", errors="replace") as stream:
        for line in stream:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            target = event.get("target", {})
            if event.get("reason") == "compiler-artifact" and target.get("name") == target_name and "lib" in target.get("kind", []):
                found.extend(name for name in event.get("filenames", []) if name.endswith(".rlib"))
    if len(found) != 1 or not Path(found[0]).is_file():
        raise ValueError("measured library rlib not uniquely available for hashing")
    return Path(found[0])


def make_job(args, root, template, toolchain, threads, concurrency, repeat, index, compiler, host):
    directory = root / f"job-{index:05d}"
    directory.mkdir()
    target = directory / "target"
    run_checked(["cp", "-a", "--reflink=auto", str(template), str(target)])
    run_checked(rustup(toolchain, "cargo", "clean", "--locked", "-p", args.crate, "--target-dir", str(target)),
                cargo_environment(args.cargo_jobs))
    profile = "check" if args.mode == "check" else "dev"
    cargo = rustup(toolchain, "cargo", "rustc", "--locked", "-vv", "--profile", profile,
                   "-p", args.crate, "--lib", "--target-dir", str(target),
                   "--message-format=json", "--", *([f"-Zthreads={threads}"] if threads is not None else []))
    unit = f"kiln-pf-{uuid.uuid4().hex[:20]}"
    # systemd-run expands ${VAR} in its arguments by default (systemd 254+), which
    # would erase the wrapper's own shell variables before /bin/sh sees them.
    scope = ["systemd-run", "--user", "--scope", "--quiet", "--expand-environment=no",
             "-p", f"CPUQuota={args.cpu_quota}",
             "-p", f"MemoryMax={args.memory_max}", "-p", "MemorySwapMax=0", "--unit", unit,
             "--", "/bin/sh", "-c", SCOPE_WRAPPER, "--", str(directory / "start.ns"),
             str(directory / "end.ns"), str(directory), *cargo]
    return {"schema": "kiln.parallel_frontend.job.v1", "toolchain": toolchain, "rustc_vv": compiler,
            "argv": scope, "cargo_argv": cargo, "crate": args.crate, "mode": args.mode,
            "threads": threads, "concurrency": concurrency, "repeat": repeat,
            "cpu_quota": args.cpu_quota, "quota_cpus": float(args.cpu_quota[:-1]) / 100,
            "memory_max": args.memory_max, "memory_swap_max": "0", "cargo_jobs": args.cargo_jobs,
            "cargo_environment": {**CONTROLLED_CARGO_ENVIRONMENT, "CARGO_BUILD_JOBS": str(args.cargo_jobs)},
            "source_revision": args.source_revision, "host_cpu_count": host[0], "host_memory_bytes": host[1],
            "job_directory": str(directory), "unit": unit, "status": "failure", "exit_status": None,
            "queue_wait_s": None, "wall_s": None, "end_to_end_s": None, "cpu_seconds": None, "peak_bytes": None,
            "peak_rss_bytes": None, "rss_samples": 0, "oom_kills": None, "stderr_tail": "", "rustc_commands": [],
            "artifact_path": None, "artifact_hash": None, "target_retained": True}


def run_batch(args, root, template, toolchain, threads, concurrency, repeat, index, compiler, host):
    jobs = [make_job(args, root, template, toolchain, threads, concurrency, repeat, index + i, compiler, host)
            for i in range(concurrency)]
    batch_id = f"{toolchain}-{threads}-{concurrency}-{repeat}"
    environment = cargo_environment(args.cargo_jobs)
    batch_start_ns = time.time_ns()
    batch_start_mono = time.monotonic_ns()
    processes = []
    for job in jobs:
        directory = Path(job["job_directory"])
        stdout = (directory / "stdout.jsonl").open("wb")
        stderr = (directory / "stderr.log").open("wb")
        try:
            process = subprocess.Popen(job["argv"], stdout=stdout, stderr=stderr, env=environment)
        except OSError as error:
            job["stderr_tail"] = str(error)
            process = None
        finally:
            stdout.close()
            stderr.close()
        processes.append(process)
    while any(process is not None and process.poll() is None for process in processes):
        for job in jobs:
            rss = sampled_scope_rss(Path(job["job_directory"]))
            if rss is not None:
                job["peak_rss_bytes"] = max(job["peak_rss_bytes"] or 0, rss)
                job["rss_samples"] += 1
        time.sleep(RSS_SAMPLE_SECONDS)
    for job, process in zip(jobs, processes):
        if process is not None:
            job["exit_status"] = process.wait()
    batch_makespan_s = (time.monotonic_ns() - batch_start_mono) / 1e9
    for job in jobs:
        directory = Path(job["job_directory"])
        started = scalar(directory / "start.ns")
        ended = scalar(directory / "end.ns")
        if started is not None and started >= batch_start_ns:
            job["queue_wait_s"] = (started - batch_start_ns) / 1e9
        if started is not None and ended is not None and ended > started:
            job["wall_s"] = (ended - started) / 1e9
            if ended >= batch_start_ns:
                job["end_to_end_s"] = (ended - batch_start_ns) / 1e9
        usage = counter(directory / "cpu.stat", "usage_usec")
        job["cpu_seconds"] = usage / 1e6 if usage is not None else None
        job["peak_bytes"] = scalar(directory / "memory.peak")
        job["oom_kills"] = counter(directory / "memory.events", "oom_kill")
        job["batch_id"] = batch_id
        job["batch_makespan_s"] = batch_makespan_s
        if (directory / "stderr.log").exists():
            job["stderr_tail"] = stderr_tail(directory / "stderr.log")
            job["rustc_commands"] = compiler_commands(directory / "stderr.log")
        job["status"] = "success" if job["exit_status"] == 0 and job["wall_s"] is not None and job["rustc_commands"] else "failure"
        if job["status"] == "success" and args.mode == "build":
            try:
                artifact = artifact_path(directory / "stdout.jsonl", args.crate)
                job["artifact_path"] = str(artifact)
                job["artifact_hash"] = subprocess.run(["b3sum", "--no-names", str(artifact)],
                                                       check=True, capture_output=True, text=True).stdout.strip()
            except (ValueError, OSError, subprocess.CalledProcessError) as error:
                job["status"] = "failure"
                job["stderr_tail"] += f"\nartifact hashing failed: {error}"
        (directory / "stdout.jsonl").unlink(missing_ok=True)
        (directory / "stderr.log").unlink(missing_ok=True)
        # Each job owns a full copy of the warm target; drop it once measured and hashed.
        shutil.rmtree(directory / "target")
        job["target_retained"] = False
    return jobs


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", required=True, help="exact dated nightly, e.g. nightly-2026-07-22")
    parser.add_argument("--baseline-toolchain", default="1.88.0")
    parser.add_argument("--crate", default="kiln-web-service")
    parser.add_argument("--threads", default=DEFAULT_THREADS, help="comma-separated subset of 1,2,4,8 including 1")
    parser.add_argument("--concurrency", default=DEFAULT_CONCURRENCY, help="at least two levels including 1")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--cpu-quota", required=True, help="per-job CPUQuota, e.g. 200%%")
    parser.add_argument("--memory-max", required=True, help="per-job MemoryMax, e.g. 4G")
    parser.add_argument("--cargo-jobs", type=int, default=None,
                        help="CARGO_BUILD_JOBS for every Cargo invocation; defaults to the host CPU count")
    parser.add_argument("--mode", choices=("check", "build"), default="check")
    parser.add_argument("--source-revision", required=True, help="exact checkout commit/revision supplied by operator")
    parser.add_argument("--out", type=Path, required=True, help="new owned output directory")
    args = parser.parse_args(argv)
    try:
        validate(args)
    except ValueError as error:
        parser.error(str(error))
    compilers = {}
    for toolchain in (args.baseline_toolchain, args.toolchain):
        result = subprocess.run(rustup(toolchain, "rustc", "-vV"), check=True, capture_output=True, text=True)
        compilers[toolchain] = result.stdout
    args.out.mkdir(mode=0o700)
    host = (os.cpu_count(), host_memory_bytes())
    rows = []
    raw = args.out / "jobs.jsonl"
    with raw.open("w", encoding="utf-8") as stream:
        for toolchain in (args.baseline_toolchain, args.toolchain):
            template = args.out / f"template-{toolchain}"
            warm = "check" if args.mode == "check" else "build"
            run_checked(rustup(toolchain, "cargo", warm, "--locked", "-p", args.crate,
                               "--lib", "--target-dir", str(template)), cargo_environment(args.cargo_jobs))
            for threads in ([None] if toolchain == args.baseline_toolchain else args.threads):
                for concurrency in args.concurrency:
                    for repeat in range(args.repeats):
                        batch = run_batch(args, args.out, template, toolchain, threads, concurrency,
                                          repeat, len(rows), compilers[toolchain], host)
                        for row in batch:
                            rows.append(row)
                            stream.write(json.dumps(row) + "\n")
                            stream.flush()
            shutil.rmtree(template)
    (args.out / "analysis.json").write_text(json.dumps(analyze(rows), indent=2) + "\n")
    return 0 if all(row["status"] == "success" for row in rows) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"parallel frontend benchmark setup failed: {error}")
