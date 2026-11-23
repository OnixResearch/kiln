# Verified UCAN with one reviewed policy file pinned by BLAKE3, two trusted
# roots and projection routes disabled.
let input = {
  schema = "kiln.implementation-authority-config.v1",
  profile = {
    verified_ucan = {
      audience = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
      generation = 7,
      policy = {
        file = {
          path = "/etc/kiln/authority/basalt-policy.json",
          blake3 = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        },
      },
      trusted_roots = [
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        "did:key:z6MktwupdmLXVVqTzCw4i46r4uGyosGXRnR3XjN4Zq7oMMsw"
      ],
      keys = [
        { did = "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK", ed25519 = "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29" },
        { did = "did:key:z6MktwupdmLXVVqTzCw4i46r4uGyosGXRnR3XjN4Zq7oMMsw", ed25519 = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a" },
      ],
      proofs_dir = "/var/lib/kiln/authority/proofs",
      revocation = { path = "/var/lib/kiln/authority/revocations.json", max_age_ms = 60000 },
      replay = { retention_ms = 604800000 },
      holders = {
        coordinator = { seed_path = "/run/credentials/kiln-implementer/coordinator.seed" },
        verifier = { seed_path = "/run/credentials/kiln-implementer/verifier.seed" },
        publisher = { seed_path = "/run/credentials/kiln-implementer/publisher.seed" },
      },
      projection_enabled = false,
    },
  },
}
in
(import "../implementation-authority.ncl") input
