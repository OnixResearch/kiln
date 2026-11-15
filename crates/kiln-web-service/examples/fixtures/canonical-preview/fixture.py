#!/usr/bin/env python3
"""Build a disposable real-source/native-worker fixture for canonical previews.

This creates owner inputs and executes one ordinary source CI run. It does not
write canonical CI state directly; source execution goes through the configured
web service, Lattice handler, and native web worker.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import time
from urllib.parse import quote
import sys

REPO = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(REPO / "scripts"))
ENGINE_ROOT = Path("/tmp/kiln-artifact-preview-trynix-7e7e793594cb58efb48279b17b4e15bbc6fbe91b")
ENGINE_MANIFEST = ENGINE_ROOT / "engine-manifest.json"
NIX = Path("/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix")
NIX_STORE = NIX.with_name("nix-store")
NIX_ID = "82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852"
PUBLIC_KEY = "trynix-examples-1:dZOV2uGWvjHo6IC5ZqCCu0dmIRzLm9pyOQJBDnBsXRY="
STORE_PATH = re.compile(r"^/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[A-Za-z0-9+._?=-]+$")


def b3(exercise, data):
    return exercise.command([exercise.exe["b3sum"], "-"], input=data).decode().split()[0]


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")
    path.chmod(0o600)


def run(argv, cwd, env, input_bytes=None):
    result = subprocess.run(argv, cwd=cwd, env=env, input=input_bytes, capture_output=True, timeout=300)
    if result.returncode:
        raise RuntimeError(f"{argv[0]} exited {result.returncode}: {result.stderr.decode(errors='replace')[:2048]}")
    if len(result.stdout) + len(result.stderr) > 1_048_576:
        raise RuntimeError(f"{argv[0]} output exceeded fixture bound")
    return result.stdout.decode().strip()


def stop(process):
    if process is not None and process.poll() is None:
        process.terminate()
        process.wait(timeout=15)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, help="new private scratch directory; retained for the run-build/produce steps")
    parser.add_argument("--evidence", required=True, help="new private evidence directory")
    parser.add_argument("--service", required=True, help="immutable kiln-web-service executable")
    parser.add_argument("--preview", required=True, help="immutable artifact-preview example executable")
    parser.add_argument("--lattice", required=True, help="immutable Lattice executable")
    parser.add_argument("--lattice-source", required=True, help="matching Lattice source tree used by the native smoke profile exporter")
    parser.add_argument("--package-root", required=True, help="exact logical signed public TryNix CLI store path")
    parser.add_argument("--store-root", required=True, help="existing private owned Nix root containing nix/store and nix/var/nix/db")
    parser.add_argument("--expires-at", type=int, required=True, help="finite Unix expiry for the preview descriptor")
    for name in ("nickel", "b3sum", "openssl", "jj", "python3", "bwrap", "bash", "true", "git", "cargo"):
        parser.add_argument("--" + name, required=True, help=f"immutable {name} executable")
    args = parser.parse_args()

    tools = {name: str(Path(getattr(args, name)).resolve(strict=True)) for name in ("nickel", "b3sum", "openssl", "jj", "python3", "bwrap", "bash", "true", "git", "cargo")}
    tools["service"] = str(Path(args.service).resolve(strict=True))
    tools["preview"] = str(Path(args.preview).resolve(strict=True))
    tools["lattice"] = str(Path(args.lattice).resolve(strict=True))
    os.environ["PATH"] = os.pathsep.join(sorted({str(Path(path).parent) for path in tools.values()}))
    args.service, args.lattice = tools["service"], tools["lattice"]
    tools["nix"] = str(NIX)
    tools["nix_store"] = str(NIX_STORE)

    if not NIX.is_file() or not NIX_STORE.is_file():
        raise FileNotFoundError("the pinned native Nix 2.34.8 executable cohort is absent")
    signed_package_root = "/nix/store/vk1ih0la8pfs92v8cy5vkv48kgmi14db-trynix-emubench-static-x86_64-unknown-linux-musl-1"
    if args.package_root != signed_package_root:
        raise ValueError("--package-root must match the exact admitted public TryNix store path")
    requested_store_root = Path(args.store_root)
    store_root = requested_store_root.resolve(strict=True)
    root_metadata = store_root.stat()
    if not requested_store_root.is_absolute() or requested_store_root != store_root or not store_root.is_dir() or root_metadata.st_uid != os.geteuid() or root_metadata.st_mode & 0o077:
        raise ValueError("--store-root must be an existing private owned normalized Nix root")
    store_uri = "local?root=" + str(store_root)
    physical_store = store_root / "nix/store"
    package = physical_store / Path(args.package_root).name
    if not STORE_PATH.fullmatch(args.package_root) or not (package / "bin/emubench").is_file():
        raise ValueError("--package-root must be the exact present signed TryNix emubench store path in the owned root")
    package_env = dict(os.environ)
    package_env["NIX_CONFIG"] = "min-free = 0"
    verify_package = [
        str(NIX), "--extra-experimental-features", "nix-command", "--store", store_uri,
        "--option", "builders", "", "--option", "substituters", "",
        "--option", "trusted-public-keys", PUBLIC_KEY,
        "store", "verify", "--recursive", "--sigs-needed", "1", args.package_root,
    ]
    subprocess.run(verify_package, env=package_env, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    package_info = json.loads(run([
        str(NIX), "--extra-experimental-features", "nix-command", "--store", store_uri,
        "path-info", "--json", "--json-format", "1", "--offline", args.package_root,
    ], REPO, package_env))[args.package_root]
    builder_roots = sorted({str(Path(tools[name]).parent.parent) for name in ("bash", "true")})
    if builder_roots:
        subprocess.run([
            str(NIX), "--extra-experimental-features", "nix-command", "--offline", "copy",
            "--to", store_uri, *builder_roots,
        ], env=package_env, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if not ENGINE_MANIFEST.is_file():
        raise FileNotFoundError(f"acquire the pinned engine first: {ENGINE_MANIFEST}")

    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("kiln_review_smoke", REPO / "scripts/review-workflow-smoke.py")
    review_smoke = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(review_smoke)
    exercise = review_smoke.ReviewExercise(argparse.Namespace(
        root=args.root, evidence=args.evidence, service=args.service,
        lattice=args.lattice, lattice_source=args.lattice_source, iroh=None,
        work=False, browser_hold=False,
    ))
    exercise.exe.update({name: tools[name] for name in tools if name in exercise.exe})
    exercise.exe["bubblewrap"] = tools["bwrap"]
    exercise.exe["true"] = str(Path(tools["true"]).parent / "true")
    exercise.exe["git"] = tools["git"]
    exercise.exe["nix"] = str(NIX)
    exercise.exe["nix_store"] = str(NIX_STORE)
    exercise.exe["cargo"] = tools["cargo"]
    root = Path(args.root).absolute()
    now = int(time.time())
    if args.expires_at <= now or args.expires_at > now + 3600:
        raise ValueError("--expires-at must be future and no more than one hour from now")
    if exercise.command([exercise.exe["b3sum"], str(NIX)]).decode().split()[0] != NIX_ID:
        raise RuntimeError("pinned Nix executable identity differs from the admitted 2.34.8 cohort")
    if exercise.command([exercise.exe["b3sum"], str(NIX_STORE)]).decode().split()[0] != NIX_ID:
        raise RuntimeError("pinned nix-store executable identity differs from the admitted 2.34.8 cohort")
    try:
        exercise.prepare()

        # Add a flake to the real disposable Git source. The derivation's src
        # input binds its result to this exact committed tree, while the only
        # runtime reference exported by the result is the already-signed CLI.
        flake = f'''{{
  description = "Kiln canonical preview source-bound selection";
  outputs = {{ self }}:
    let
      src = self.outPath;
      sourceRevision = self.rev;
      # These exact dependencies were admitted and copied into the owned store.
      pinnedStore = path: builtins.appendContext path {{ ${{path}} = {{ path = true; }}; }};
      cli = pinnedStore {json.dumps(args.package_root)};
      bash = pinnedStore {json.dumps(str(Path(tools["bash"]).parent.parent))};
      coreutils = pinnedStore {json.dumps(str(Path(tools["true"]).parent.parent))};
      shell = bash + "/bin/bash";
      mkdir = coreutils + "/bin/mkdir";
      ln = coreutils + "/bin/ln";
    in {{
      packages.x86_64-linux.default = derivation {{
        name = "kiln-canonical-preview-artifact";
        system = "x86_64-linux";
        builder = shell;
        args = [ "-e" "-c" ''
          test -f "$src/ci.py"
          {"${mkdir}"} -p "$out/bin"
          {"${ln}"} -s {"${cli}"}/bin/emubench "$out/bin/emubench"
          printf '%s\\n' "$sourceRevision" > "$out/source-revision"
        '' ];
        inherit src sourceRevision;
      }};
    }};
}}
'''
        (exercise.repo / "flake.nix").write_text(flake)
        exercise.command([exercise.exe["jj"], "-R", exercise.repo, "file", "track", 'root:"flake.nix"'])
        exercise.command([exercise.exe["jj"], "-R", exercise.repo, "describe", "-m", "Add source-bound canonical preview flake"])
        exercise.command([exercise.exe["jj"], "-R", exercise.repo, "bookmark", "set", "main", "-r", "@"])
        revision = exercise.command([exercise.exe["jj"], "-R", exercise.repo, "log", "--no-graph", "-r", "main", "-T", "commit_id"]).decode().strip()
        if len(revision) != 40 or any(ch not in "0123456789abcdef" for ch in revision):
            raise RuntimeError("native source revision was not a Git commit identity")
        baseline_revision = revision
        candidate_source = exercise.branch("canonical-preview")
        if candidate_source == baseline_revision:
            raise RuntimeError("review source candidate did not differ from its frozen base")
        revision = candidate_source

        flake_ref = "git+file://" + quote(str(exercise.repo), safe="/") + "?ref=refs/heads/review/canonical-preview&rev=" + revision
        nix_eval = [
            str(NIX), "--extra-experimental-features", "nix-command flakes", "--store", store_uri,
            "--option", "builders", "", "--option", "substituters", "", "--offline", "eval", "--raw",
        ]
        nix_env = dict(exercise.env)
        nix_env["NIX_CONFIG"] = "min-free = 0"
        drv = run([*nix_eval, flake_ref + "#packages.x86_64-linux.default.drvPath"], exercise.repo, nix_env)
        out = run([*nix_eval, flake_ref + "#packages.x86_64-linux.default.outPath"], exercise.repo, nix_env)
        drv_identity = exercise.command([exercise.exe["b3sum"], physical_store / Path(drv).name]).decode().split()[0]

        profile_export = exercise.command([exercise.exe["nickel"], "export", "--format", "json", REPO / "config/nix-build-profile.ncl"])
        nix_profile = json.loads(profile_export)["profile"]
        nix_profile.update(
            derivation_path=drv,
            derivation_identity=drv_identity,
            plan_identity=b3(exercise, ("kiln.preview.fixture.plan.v1|" + revision).encode()),
            policy_identity=b3(exercise, ("kiln.preview.fixture.policy.v1|" + revision).encode()),
            worker_identity=b3(exercise, ("kiln.preview.fixture.nix-worker.v1|" + str(NIX) + "|" + NIX_ID).encode()),
            store_identity=b3(exercise, ("kiln.preview.fixture.store.v1|/nix/store|" + str(store_root)).encode()),
            store_root=str(store_root),
            expected_outputs=[{"name": "out", "path": out}],
            gc_root_base=str(root / "canonical-nix-roots"),
            home_base=str(root / "canonical-nix-home"),
        )
        for directory in (root / "canonical-nix-roots", root / "canonical-nix-home"):
            directory.mkdir(mode=0o700, exist_ok=True)
        service_profile = root / "service.json"
        service = json.loads(service_profile.read_text())
        bootstrap_state_directory = service["state_directory"]
        project = next(project for project in service["projects"] if project["id"] == "success")
        project["admitted_references"].append("refs/heads/review/canonical-preview")
        project["canonical_nix_build"] = {
            "node_id": "native-build",
            "source_revision": revision,
            "profile": nix_profile,
        }
        # Freeze the source-bound build profile before admitting any review CI.
        # The default bootstrap Engine profile is already bound to its store.
        # A new store executes and retains the actual current review candidate.
        if hasattr(exercise, "heartbeats"):
            for heartbeat in exercise.heartbeats.values():
                stop(heartbeat)
            del exercise.heartbeats
        stop(exercise.service)
        canonical_state = root / "canonical-state"
        canonical_state.mkdir(mode=0o700)
        service["state_directory"] = str(canonical_state)
        write_json(service_profile, service)
        exercise.start_service()
        exercise.enroll("success")
        exercise.start_heartbeat()
        configured_source = exercise.terminal(exercise.api("POST", "/api/runs", {
            "project_id": "success",
            "request_id": "canonical-preview-configured-source",
            "reference": "refs/heads/review/canonical-preview",
            "source_revision": revision,
        }))
        if configured_source.get("outcome") != "Success" or configured_source.get("source_revision") != revision:
            raise RuntimeError("configured canonical store did not execute successful exact-source CI")
        source_run_id = configured_source["id"]

        intake = json.loads(exercise.cli(
            "review-submit", "--request-id", "canonical-preview-review-intake",
            "--source-ref", "refs/heads/review/canonical-preview",
            "--title", "Canonical preview fixture",
            "--description", "Source-bound native CI and exact artifact preview",
        ))
        change_id = intake["change"]["id"]
        exercise.review_command({"type": "vote", "value": "approve"}, change_id, reviewer=True)
        exercise.review_command({"type": "prepare_submit"}, change_id)
        native_candidate = exercise.detail(change_id)["change"].get("candidate")
        if not native_candidate or native_candidate.get("commit") != revision or native_candidate.get("ci_run") != source_run_id:
            raise RuntimeError("current native review candidate did not select the exact successful CI run")

        secret_key = root / "canonical-preview-cache.secret"
        public_key_file = root / "canonical-preview-cache.pub"
        exercise.command([str(NIX_STORE), "--generate-binary-cache-key", "kiln-canonical-preview-fixture", secret_key, public_key_file])
        secret_key.chmod(0o600)
        keys = sorted([PUBLIC_KEY, public_key_file.read_text().strip()])
        selection_now = int(time.time())
        if args.expires_at <= selection_now or args.expires_at > selection_now + 3600:
            raise ValueError("--expires-at expired or exceeds the one-hour descriptor bound during fixture setup")
        trust_identity = b3(exercise, ("kiln.preview.canonical-fixture-trust.v1|" + "|".join(keys)).encode())
        selection = {
            "root_path": out,
            "executable": out + "/bin/emubench",
            "arguments": ["noop"],
            "trust_identity": trust_identity,
            "public_keys": keys,
            "limits": {
                "runtime_bytes": 16_777_216,
                "run_seconds": 120,
                "output_bytes": 16_384,
                "expires_at": args.expires_at,
                "retain_after_session": False,
            },
        }
        selection_path = root / "selection.json"
        write_json(selection_path, selection)


        preview_scratch = root / "preview-scratch"
        preview_scratch.mkdir(mode=0o700)
        for process in reversed(exercise.processes):
            stop(process)
        if exercise.notification_server is not None:
            exercise.notification_server.shutdown()
            exercise.notification_server.server_close()
            exercise.notification_thread.join(timeout=5)
        tool_inputs = {
            name: {
                "path": path,
                "blake3": exercise.command([exercise.exe["b3sum"], path]).decode().split()[0],
            }
            for name, path in sorted(exercise.exe.items())
        }
        tool_inputs["artifact_preview"] = {
            "path": tools["preview"],
            "blake3": exercise.command([exercise.exe["b3sum"], tools["preview"]]).decode().split()[0],
        }
        lattice_revision = run(
            [exercise.exe["git"], "-C", args.lattice_source, "rev-parse", "HEAD"],
            root, exercise.env,
        )
        worker_profile = root / "worker-success.json"
        worker_profile_identity = exercise.digest(worker_profile)
        review_profile = root / "review.json"
        service_command = [
            tools["service"], "serve", "--profile", str(service_profile),
            "--access", str(root / "access.json"),
            "--automation", str(root / "automation.json"),
            "--cache", str(root / "cache.json"),
            "--workers", str(root / "workers.json"),
            "--code-review", str(review_profile),
            "--artifact-previews", str(root / "preview-catalog.json"),
        ]
        manifest = json.loads(ENGINE_MANIFEST.read_text())
        result = {
            "fixture_command": [tools["python3"], str(Path(__file__).resolve()), *sys.argv[1:]],
            "schema": "kiln.canonical-preview-native-fixture.v1",
            "service_profile": str(service_profile),
            "review_profile": str(review_profile),
            "review_change_id": change_id,
            "source_run_id": source_run_id,
            "candidate_ci_run_id": source_run_id,
            "bootstrap_state_directory": bootstrap_state_directory,
            "canonical_state_directory": str(canonical_state),
            "source_revision": revision,
            "source_flake_ref": flake_ref,
            "signed_package_cache": "https://trynix.dev/examples/cache",
            "signed_package_nar_size": package_info["narSize"],
            "signed_package_nar_sha256_sri": package_info["narHash"],
            "candidate_commit": native_candidate["commit"],
            "build_node": "native-build",
            "worker_profile": str(worker_profile),
            "worker_profile_blake3": worker_profile_identity,
            "lattice_source_revision": lattice_revision,
            "tool_inputs": tool_inputs,
            "selection": str(selection_path),
            "preview_scratch": str(root / "preview-scratch"),
            "catalog": str(root / "preview-catalog.json"),
            "engine_root": str(ENGINE_ROOT),
            "engine_manifest": str(ENGINE_MANIFEST),
            "engine_manifest_blake3": exercise.command([exercise.exe["b3sum"], str(ENGINE_MANIFEST)]).decode().split()[0],
            "engine_source_revision": manifest["source_revision"],
            "engine_identity": "b114a874c62fdb1d2d74df0af625a433f782ea841ac558f925bdd96d4abd25e5",
            "store_root": str(store_root),
            "preview": tools["preview"],
            "sign_output_command": ["env", "NIX_CONFIG=min-free = 0", str(NIX), "--extra-experimental-features", "nix-command", "--store", store_uri, "store", "sign", "--key-file", str(secret_key), out],
            "run_build_command": [tools["preview"], "run-build", str(service_profile), source_run_id, "canonical-preview-build-" + revision[:12]],
            "produce_command_template": [tools["preview"], "produce", str(service_profile), "<BUILD_RUN_ID>", "native-build", str(selection_path), str(ENGINE_ROOT), str(ENGINE_MANIFEST), str(NIX), str(NIX_STORE), str(root / "preview-scratch"), str(root / "preview-catalog.json")],
            "produce_environment": {"KILN_PREVIEW_STORE_ROOT": str(store_root)},
            "review_serve_command": service_command,
            "nix": str(NIX),
            "nix_store": str(NIX_STORE),
            "derivation_path": drv,
            "product_path": out,
            "signed_package_root": args.package_root,
            "signed_package_key": PUBLIC_KEY,
        }
        write_json(root / "fixture.json", result)
        print(json.dumps(result, indent=2))
    except BaseException:
        # The owner-supplied scratch, service state, and credentials are retained
        # for diagnosis and are never removed by the fixture constructor.
        for process in reversed(exercise.processes):
            try:
                stop(process)
            except Exception:
                pass
        raise


if __name__ == "__main__":
    os.umask(0o077)
    main()
