# Trusted roots are unique.
let input = {
  schema = "kiln.implementation-authority-config.v1",
  profile = {
    verified_ucan = {
      audience = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
      generation = 1,
      policy = 'packaged,
      trusted_roots = [
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK"
      ],
      keys = [
        { did = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK", ed25519 = "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29" },
      ],
      proofs_dir = "/var/lib/kiln/authority/proofs",
      revocation = { path = "/var/lib/kiln/authority/revocations.json", max_age_ms = 300000 },
      replay = { retention_ms = 86400000 },
      holders = {
        coordinator = { seed_path = "/run/credentials/kiln-implementer/coordinator.seed" },
        verifier = { seed_path = "/run/credentials/kiln-implementer/verifier.seed" },
        publisher = { seed_path = "/run/credentials/kiln-implementer/publisher.seed" },
      },
      projection_enabled = true,
    },
  },
}
in
(import "../implementation-authority.ncl") input
