"""Real native Firecracker compatibility rail, separate from hosted confinement.

Build the three flake image outputs and unikernel-guest first. This script never
provisions a host, starts a network device, uses an API socket, or claims the
hosted ConfinementObserved gate. Hosted execution must exercise that gate itself.
"""
import argparse
import json
import pathlib
import selectors
import subprocess
import time

from image import digest, image

TIMEOUT_SECONDS = 10
OUTPUT_LIMIT = 1024 * 1024
RESULT_PREFIX = "KILN_RESULT:"


def load(path):
    return json.loads((pathlib.Path(path) / "manifest.json").read_text())


def check_file(record):
    if digest(record["path"]) != record["blake3"]:
        raise ValueError("manifest content identity mismatch")


def status(serial):
    frames = [line[len(RESULT_PREFIX):] for line in serial.splitlines() if line.startswith(RESULT_PREFIX)]
    if len(frames) != 1:
        raise ValueError("missing or ambiguous native exit status")
    frame = json.loads(frames[0])
    if set(frame) != {"schema", "exit_code"} or frame["schema"] != "kiln.unikernel.result.v1":
        raise ValueError("invalid native result frame")
    if type(frame["exit_code"]) is not int or not 0 <= frame["exit_code"] <= 255:
        raise ValueError("invalid native exit code")
    return frame["exit_code"]


def boot(vmm, kernel, initrd, argv, out, name, marker):
    config = {"boot-source": {"kernel_image_path": kernel, "initrd_path": initrd,
              "boot_args": "elfloader /test" + "".join(" " + arg for arg in argv) + " KILN_ARGV_END"},
              "drives": [], "machine-config": {"vcpu_count": 1, "mem_size_mib": 256}}
    # No NIC or vsock is configured. This raw rail is not the hosted observer gate.
    config_path = out / (name + ".config.json")
    config_path.write_text(json.dumps(config, sort_keys=True))
    started = time.monotonic()
    process = subprocess.Popen([vmm, "--no-api", "--config-file", str(config_path)],
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, env={})
    streams = {process.stdout: bytearray(), process.stderr: bytearray()}
    selector = selectors.DefaultSelector()
    for stream in streams:
        selector.register(stream, selectors.EVENT_READ)
    first_application_ms = None
    try:
        while selector.get_map():
            remaining = TIMEOUT_SECONDS - (time.monotonic() - started)
            if remaining <= 0:
                raise TimeoutError("native guest exceeded deadline")
            for key, _ in selector.select(remaining):
                chunk = key.fileobj.read1(4096)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                streams[key.fileobj].extend(chunk)
                if sum(map(len, streams.values())) > OUTPUT_LIMIT:
                    raise ValueError("native output limit exceeded")
                if first_application_ms is None and marker.encode() in streams[process.stdout]:
                    first_application_ms = round((time.monotonic() - started) * 1000, 3)
        returncode = process.wait(timeout=max(0.001, TIMEOUT_SECONDS - (time.monotonic() - started)))
        serial = streams[process.stdout].decode("utf-8", errors="strict")
        (out / (name + ".serial.log")).write_text(serial)
        (out / (name + ".vmm.log")).write_bytes(streams[process.stderr])
        if returncode != 0:
            raise ValueError("VMM did not exit successfully")
        return {"name": name, "exit_code": status(serial), "serial": serial,
                "cold_boot_to_application_ms": first_application_ms,
                "spawn_to_exit_ms": round((time.monotonic() - started) * 1000, 3),
                "network_device_configured": False, "vsock_device_configured": False}
    finally:
        selector.close()
        if process.poll() is None:
            process.kill()
            process.wait()
        for stream in streams:
            stream.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--guest", required=True)
    parser.add_argument("--rust-image", required=True)
    parser.add_argument("--go-image", required=True)
    parser.add_argument("--probe-image", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    out = pathlib.Path(args.output).resolve()
    out.mkdir(parents=True, exist_ok=False)
    guest = load(args.guest)
    for record in (guest["kernel"], guest["firecracker"]):
        check_file(record)
    measurements = []
    repeats = []
    for language, location in (("rust", args.rust_image), ("go", args.go_image), ("probe", args.probe_image)):
        manifest = load(location)
        for field in ("binary", "kernel", "image"):
            check_file(manifest[field])
        if manifest["kernel"]["blake3"] != guest["kernel"]["blake3"]:
            raise ValueError("fixture kernel differs from reviewed guest")
        # Fixture flake attrs have no extra files. General fixture images remain
        # supported by image.py and the hosted runner, not guessed by this rail.
        if manifest["fixtures"]:
            raise ValueError("compatibility fixture unexpectedly has extra files")
        first = image(manifest["binary"]["path"], [])
        second = image(manifest["binary"]["path"], [])
        if first != second or first != pathlib.Path(manifest["image"]["path"]).read_bytes():
            raise ValueError("native image was not reproduced from declared inputs")
        repeats.append({"name": language, "image_blake3": manifest["image"]["blake3"], "identical_rebuilds": 2})
        for mode, expected in (("pass", 0), ("fail", 7)) if language != "probe" else (("probe", 0),):
            observation = boot(guest["firecracker"]["path"], guest["kernel"]["path"],
                               manifest["image"]["path"], [] if language == "probe" else [mode],
                               out, language + "-" + mode,
                               "native network and process denial probe passed" if language == "probe" else "static " + ("Rust" if language == "rust" else "Go") + " native test")
            if observation["exit_code"] != expected or observation["cold_boot_to_application_ms"] is None:
                raise ValueError("native fixture did not reach its expected outcome")
            if language == "probe":
                for marker in ("AF_INET: socket=-1 connect=-1", "AF_VSOCK CID 1: socket=-1 connect=-1",
                               "AF_VSOCK CID 2: socket=-1 connect=-1", "fork=-1", "execve=-1"):
                    if marker not in observation["serial"]:
                        raise ValueError("native denial probe did not observe " + marker)
            observation.pop("serial")
            measurements.append(observation)
        if language == "rust":
            fixture_source = out / "readonly.txt"
            fixture_source.write_bytes(b"immutable fixture\n")
            declared = [{"path": "readonly.txt", "source": str(fixture_source)}]
            fixture_image = image(manifest["binary"]["path"], declared)
            if fixture_image != image(manifest["binary"]["path"], declared):
                raise ValueError("declared-fixture image was not deterministic")
            fixture_path = out / "rust-fixture.cpio"
            fixture_path.write_bytes(fixture_image)
            observation = boot(guest["firecracker"]["path"], guest["kernel"]["path"],
                               str(fixture_path), ["fixture"], out, "rust-fixture",
                               "declared fixture read succeeds; write denied; bytes unchanged")
            if observation["exit_code"] != 0 or observation["cold_boot_to_application_ms"] is None:
                raise ValueError("native fixture read/write protection failed")
            observation.pop("serial")
            measurements.append(observation)
            repeats.append({"name": "rust-fixture", "image_blake3": digest(fixture_path), "identical_rebuilds": 2})
            fixture_path.unlink()
    receipt = {"schema": "kiln.unikernel.native-smoke.v1", "guest": guest,
               "measurements": measurements, "image_rebuilds": repeats,
               "single_host_measurement": True, "hosted_confinement_observed": False,
               "hostile_isolation_claim": False}
    (out / "receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(json.dumps(receipt, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
