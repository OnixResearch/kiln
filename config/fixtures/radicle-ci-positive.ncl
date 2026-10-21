# Isolated configuration conformance only; not an operator deployment.
let Status = (import "../radicle-status-profile.ncl").profile in
(import "../radicle-ci.ncl") {
  worker_profile_path = "/var/lib/kiln/project.json",
  worker_profile_identity = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  socket_path = "/var/lib/kiln/aspen.sock",
  status_storage_root = "/var/lib/kiln/status",
  status = {
    allowed_rids = Status.allowed_rids,
    signer_identity = Status.signer_identity & { contract_version = 1 },
    signer = {
      project_root = Status.signer.project_root,
      project = Status.signer.project,
      profile = Status.signer.profile,
      secret_key = Status.signer.secret_key,
      provider = Status.signer.provider,
    },
  },
}
