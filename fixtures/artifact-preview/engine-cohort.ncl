# Real already-built public static CLI. This is engine-only qualification, not
# a fabricated Kiln run/candidate/receipt. Pass a finite Unix expiry explicitly.
# Trust identity is BLAKE3 over exactly:
# kiln.preview.public-engine-cohort.v1|trynix-examples-1:dZOV2uGWvjHo6IC5ZqCCu0dmIRzLm9pyOQJBDnBsXRY=
fun expiry => (import "../../config/artifact-preview.ncl") {
  root_path = "/nix/store/vk1ih0la8pfs92v8cy5vkv48kgmi14db-trynix-emubench-static-x86_64-unknown-linux-musl-1",
  executable = "/nix/store/vk1ih0la8pfs92v8cy5vkv48kgmi14db-trynix-emubench-static-x86_64-unknown-linux-musl-1/bin/emubench",
  arguments = ["noop"],
  trust_identity = "7852f9febff8b2acaa27dc59286f648f8793d120600380416940717b7a6ae4d2",
  public_keys = ["trynix-examples-1:dZOV2uGWvjHo6IC5ZqCCu0dmIRzLm9pyOQJBDnBsXRY="],
  limits = { expires_at = expiry, runtime_bytes = 2097152, run_seconds = 120, output_bytes = 16384, retain_after_session = false },
}
