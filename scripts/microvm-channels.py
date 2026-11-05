#!/usr/bin/env python3
"""Opt-in cooperative guest timing qualification, never an isolation CI gate.

build creates a static /init and minimal initramfs; worker runs one disposable
networkless Firecracker VM. run drives local or SSH worker argv ports from JSON.
No sender bits enter a receiver request. Raw samples and held-out rates persist.
"""
import argparse
import concurrent.futures
from contextlib import contextmanager
import gzip
import json
import hashlib
import hmac
import os
from pathlib import Path
import secrets
import signal
import statistics
import subprocess
import sys
import tempfile
import threading
import time

PREFIX = "KILN_PROBE "
TRAINING = "01" * 16
CHANNELS = ("cache", "cpu", "memory")
PLACEMENTS = ("same_host", "pinned_cores", "separate_pools")
MAX_SYMBOLS = 1024 + len(TRAINING)
MAX_SYMBOL_MS = 1000
MAX_LEAD_MS = 30000
GUEST_FINISH_MARGIN_SECONDS = 10
WORKER_MARGIN_SECONDS = 5
CANCELLED = threading.Event()


def schedule_timeout(request, now_ms=None):
    now_ms = time.time()*1000 if now_ms is None else now_ms
    count = len(request["bits"]) if request["role"] == "sender" else request["count"]
    slot, lead = request["symbol_ms"], request["start_ms"] - now_ms
    if type(count) is not int or not 4 <= count <= MAX_SYMBOLS or type(slot) is not int or not 20 <= slot <= MAX_SYMBOL_MS or not 0 <= lead <= MAX_LEAD_MS:
        raise ValueError("probe schedule outside admitted bounds")
    return lead/1000 + count*slot/1000 + GUEST_FINISH_MARGIN_SECONDS


@contextmanager
def owned_process(command, **options):
    process = subprocess.Popen(command, start_new_session=True, **options)
    try:
        yield process
    finally:
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=WORKER_MARGIN_SECONDS)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
        for stream in (process.stdin, process.stdout, process.stderr):
            if stream is not None:
                stream.close()


def interrupt(signum, _frame):
    CANCELLED.set()
    raise KeyboardInterrupt(f"measurement cancelled by signal {signum}")


def os_instance_label(key, machine_id):
    if machine_id is None:
        return None
    # Report-scoped privacy pseudonym, not a content identity or hardware proof.
    return hmac.new(bytes.fromhex(key), machine_id.encode(), hashlib.sha256).hexdigest()


def archive_entry(name, mode, data=b"", major=0, minor=0):
    fields = (0, mode, 0, 0, 1, 0, len(data), 0, 0, major, minor, len(name)+1, 0)
    header = b"070701" + b"".join(f"{field:08x}".encode() for field in fields)
    entry = header + name.encode() + b"\0"
    entry += b"\0" * (-len(entry) % 4)
    return entry + data + b"\0" * (-len(data) % 4)


def build(args):
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = output / "probe"
    source = Path(__file__).resolve().parents[1] / "tools/microvm-channel-probe.c"
    subprocess.run([args.cc, "-O2", "-static", "-Wall", "-Wextra", "-Werror", str(source), "-o", str(binary)], check=True)
    archive = archive_entry("dev", 0o040755) + archive_entry("proc", 0o040755)
    archive += archive_entry("dev/console", 0o020600, major=5, minor=1)
    archive += archive_entry("init", 0o100755, binary.read_bytes())
    archive += archive_entry("TRAILER!!!", 0)
    (output / "probe-initrd.gz").write_bytes(gzip.compress(archive, mtime=0))
    print(json.dumps({"probe":str(binary),"initrd":str(output / "probe-initrd.gz")}))


def read_fact(path):
    try:
        return Path(path).read_text()[:65536].strip()
    except OSError:
        return None


def worker(_args):
    request = json.load(sys.stdin)
    timeout = schedule_timeout(request)
    role, channel = request["role"], request["channel"]
    if role not in ("sender", "receiver") or channel not in CHANNELS:
        raise ValueError("invalid probe role/channel")
    start, slot = request["start_ms"], request["symbol_ms"]
    payload = request["bits"] if role == "sender" else str(request["count"])
    if role == "sender" and (not payload or set(payload) - {"0", "1"}):
        raise ValueError("invalid sender payload")
    count = len(payload) if role == "sender" else int(payload)
    endpoint = request["endpoint"]
    cpus = endpoint.get("cpus")
    topology = {}
    if cpus is not None:
        if not isinstance(cpus, list) or not cpus or len(set(cpus)) != len(cpus) or any(type(cpu) is not int or not 0 <= cpu < 4096 for cpu in cpus):
            raise ValueError("cpus must be explicit bounded CPU numbers")
        for cpu in cpus:
            root = f"/sys/devices/system/cpu/cpu{cpu}/topology"
            topology[str(cpu)] = {key:read_fact(f"{root}/{key}") for key in ("physical_package_id", "core_id", "thread_siblings_list")}
            for index in range(8):
                cache = f"/sys/devices/system/cpu/cpu{cpu}/cache/index{index}"
                if read_fact(f"{cache}/level") == "3":
                    topology[str(cpu)]["llc_shared_cpus"] = read_fact(f"{cache}/shared_cpu_list")
    boot = f"console=ttyS0 reboot=k panic=1 pci=off init=/init kiln_probe={role},{channel},{start},{slot},{payload}"
    with tempfile.TemporaryDirectory(prefix="kiln-channel-") as directory:
        config = {"boot-source":{"kernel_image_path":endpoint["kernel"],"initrd_path":endpoint["initrd"],"boot_args":boot},"machine-config":{"vcpu_count":1,"mem_size_mib":512,"smt":False}}
        config["drives"] = []
        config_path = Path(directory)/"config.json"
        config_path.write_text(json.dumps(config))
        command = [endpoint["firecracker"], "--no-api", "--config-file", str(config_path), "--level", "Error"]
        if cpus is not None:
            command = [endpoint.get("taskset", "taskset"), "-c", ",".join(map(str,cpus))] + command
        with tempfile.TemporaryFile() as log, owned_process(command, stdout=log, stderr=subprocess.STDOUT) as process:
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                log.seek(0)
                raise RuntimeError(f"guest probe timed out: {log.read(65536).decode(errors='replace')[-8000:]}")
            log.seek(0)
            serial = log.read(2*1024*1024+1)
        if len(serial)>2*1024*1024:
            raise RuntimeError("guest serial exceeds evidence bound")
        serial = serial.decode(errors="replace")
        rows = [line[line.index(PREFIX)+len(PREFIX):] for line in serial.splitlines() if PREFIX in line]
        # Firecracker 1.15.1 reports guest reboot as GenericError/exit 1.
        # Admit that documented terminal path only alongside a complete probe.
        rebooted = process.returncode == 1 and "reboot: machine restart" in serial and "Unexpected exit reason on vcpu run: Shutdown" in serial
        if (process.returncode != 0 and not rebooted) or len(rows)!=1:
            raise RuntimeError(f"guest failed ({process.returncode}): {serial[-8000:]}")
        try:
            probe = json.loads(rows[0])
        except json.JSONDecodeError as error:
            raise RuntimeError(f"malformed guest probe: {serial[-8000:]}") from error
        if probe["role"] != role or probe["channel"] != channel or len(probe["samples"]) != count:
            raise RuntimeError("guest probe result identity mismatch")
        print(json.dumps({"probe":probe,"vmm_exit_code":process.returncode,"guest_reboot_observed":rebooted,"os_instance_label":os_instance_label(request["identity_key"],read_fact("/etc/machine-id")),"identity_non_claim":"Report-scoped OS-instance pseudonym; not physical-host attestation","kernel_release":read_fact("/proc/sys/kernel/osrelease"),"smt_active":read_fact("/sys/devices/system/cpu/smt/active"),"ksm_run":read_fact("/sys/kernel/mm/ksm/run"),"cpus":cpus,"topology":topology,"endpoint":endpoint,"serial":serial}))


def call_worker(endpoint, request):
    # argv is an explicit operator port, e.g. ["ssh", "pool-b", "python3",
    # "/opt/kiln/scripts/microvm-channels.py", "worker"]. No shell interpolation.
    request = dict(request, endpoint=endpoint)
    timeout = schedule_timeout(request) + WORKER_MARGIN_SECONDS
    deadline = time.monotonic() + timeout
    with owned_process(endpoint["command"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as process:
        pending_input = json.dumps(request)
        while True:
            if CANCELLED.is_set():
                raise RuntimeError("measurement cancelled")
            remaining = deadline-time.monotonic()
            if remaining <= 0:
                raise TimeoutError("measurement worker exceeded admitted schedule")
            try:
                stdout, stderr = process.communicate(input=pending_input, timeout=min(0.2,remaining))
                break
            except subprocess.TimeoutExpired:
                pending_input = None
        if process.returncode:
            raise RuntimeError(f"worker failed: {stderr[-10000:]}")
        return json.loads(stdout)


def reduce_samples(bits, samples, slot):
    if len(samples)!=len(bits) or len(bits)<=len(TRAINING) or bits[:len(TRAINING)]!=TRAINING:
        raise ValueError("probe samples do not match training and held-out payload")
    zero = statistics.median(samples[i] for i, bit in enumerate(TRAINING) if bit=="0")
    one = statistics.median(samples[i] for i, bit in enumerate(TRAINING) if bit=="1")
    threshold = (zero+one)/2
    decoded = "".join("1" if (sample>threshold if one>zero else sample<threshold) else "0" for sample in samples[len(TRAINING):])
    expected = bits[len(TRAINING):]
    errors = sum(a!=b for a,b in zip(expected,decoded))
    return {"raw_bitrate_bps":1000/slot,"bit_error_rate":errors/len(expected),"bit_errors":errors,"held_out_symbols":len(expected),"training_zero_median":zero,"training_one_median":one,"threshold":threshold,"decoded":decoded,"channel_detected":None,"non_claim":"Measured cooperative error, not channel capacity or absence proof"}


def validate_placement(name, sender, receiver):
    left, right = sender["os_instance_label"], receiver["os_instance_label"]
    if not left or not right:
        raise ValueError("OS-instance identity unavailable")
    if name == "separate_pools":
        pools = [result["endpoint"].get("pool") for result in (sender,receiver)]
        if not all(pools) or pools[0] == pools[1]:
            raise ValueError("separate pools require distinct declared runner pool identities")
        if left == right:
            def llcs(result):
                if not result["cpus"]:
                    raise ValueError("same-host separate pools require explicit CPU sets")
                values = {(v["physical_package_id"],v.get("llc_shared_cpus")) for v in result["topology"].values()}
                if any(None in value for value in values):
                    raise ValueError("LLC topology unavailable")
                return values
            if llcs(sender) & llcs(receiver):
                raise ValueError("separate same-host pools share an LLC")
            return "distinct_llc_same_observed_os_instance_measurement_only"
        return "distinct_observed_os_instances_physical_placement_unverified"
    elif left!=right:
        raise ValueError("same-instance placement returned distinct OS instances")
    if name == "pinned_cores":
        def cores(result):
            if not result["cpus"]:
                raise ValueError("pinned placement requires explicit CPU sets")
            values = {(v["physical_package_id"],v["core_id"]) for v in result["topology"].values()}
            if any(None in value for value in values):
                raise ValueError("physical core topology unavailable")
            return values
        if cores(sender) & cores(receiver):
            raise ValueError("pinned placement shares a physical core")
    return "same_observed_os_instance_physical_placement_unverified"


def run(args):
    config = json.loads(Path(args.config).read_text())
    identity_key = secrets.token_hex(32)
    report = {"schema":"kiln.microvm.channel-measurement.v2","hostile_isolation_claim":False,"runs":[],"unavailable":{}}
    output = Path(args.output)
    if output.exists():
        raise ValueError("refusing to overwrite retained measurement evidence")
    def retain():
        output.write_text(json.dumps(report,indent=2)+"\n")
    for name in PLACEMENTS:
        if name not in config:
            report["unavailable"][name] = "Requires operator-provisioned endpoints, matching probe images and synchronized clocks; OS-instance labels alone cannot establish physical separation, while local distinct LLC pools require explicit pinning and observed disjoint topology"
            continue
        for channel in CHANNELS:
            bits = TRAINING + "".join(str(secrets.randbits(1)) for _ in range(args.symbols))
            start = int(time.time()*1000)+args.lead_ms
            common = {"channel":channel,"start_ms":start,"symbol_ms":args.symbol_ms,"identity_key":identity_key}
            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                send = pool.submit(call_worker,config[name]["sender"],dict(common,role="sender",bits=bits))
                receive = pool.submit(call_worker,config[name]["receiver"],dict(common,role="receiver",count=len(bits)))
                sender,receiver = send.result(),receive.result()
            separation = validate_placement(name,sender,receiver)
            result = reduce_samples(bits,receiver["probe"]["samples"],args.symbol_ms)
            report["runs"].append({"placement":name,"separation_observed":separation,"channel":channel,"sender_bits":bits,"measurement":result,"sender":sender,"receiver":receiver})
            retain()
    retain()
    print(json.dumps({"output":str(output),"observed_runs":len(report["runs"]),"unavailable":report["unavailable"]}))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest="action",required=True)
    p=sub.add_parser("build"); p.add_argument("--cc",default="cc"); p.add_argument("--output",required=True); p.set_defaults(function=build)
    p=sub.add_parser("worker"); p.set_defaults(function=worker)
    p=sub.add_parser("run"); p.add_argument("--config",required=True); p.add_argument("--output",required=True); p.add_argument("--symbols",type=int,default=64); p.add_argument("--symbol-ms",type=int,default=100); p.add_argument("--lead-ms",type=int,default=5000); p.set_defaults(function=run)
    args=parser.parse_args()
    for signum in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(signum, interrupt)
    if args.action=="run" and (not 8<=args.symbols<=1024 or not 20<=args.symbol_ms<=1000 or not 1000<=args.lead_ms<=30000):
        parser.error("probe bounds: symbols 8..1024, symbol-ms 20..1000, lead-ms 1000..30000")
    args.function(args)

if __name__=="__main__":
    main()
