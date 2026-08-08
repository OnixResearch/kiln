let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Hash = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == 64
) in
let Commit = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == 40
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let Receipt = {
  schema | NonEmpty,
  status | NonEmpty,
  authority | NonEmpty,
  publication_sequence | Positive,
  observed_at_utc | NonEmpty,
  rid | NonEmpty,
  source_revision | Commit,
  source_archive_blake3 | Hash,
  source_archive_bytes | Positive,
  hosting_profile_blake3 | Hash,
  license | NonEmpty,
  identity_revision | Commit,
  delegates | Array NonEmpty,
  threshold | Positive,
  author_sigrefs | Commit,
  signed_refs_feature | NonEmpty,
  branches | {
    main | Commit,
    candidate | Commit,
  },
  native_acquisition | {
    seed | NonEmpty,
    signed_refs_feature | NonEmpty,
    revision | Commit,
    archive_blake3 | Hash,
    git_fsck | NonEmpty,
  },
  replication | {
    required | Positive,
    observed_in_sync | Positive,
  },
  https_acquisition | {
    url | NonEmpty,
    revision | Commit,
    archive_blake3 | Hash,
    git_fsck | NonEmpty,
    unknown_ref | NonEmpty,
    receive_pack_discovery | NonEmpty,
    receive_pack_post | NonEmpty,
  },
  non_claims | Array NonEmpty,
} in
{
  receipt | Receipt = {
    schema = "kiln.radicle-publication-receipt.v1",
    status = "published",
    authority = "operator_authorized",
    publication_sequence = 1,
    observed_at_utc = "2026-08-08T05:32:03Z",
    rid = "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
    source_revision = "36984e367eafd44c22d2b4f4f47121deb51eece3",
    source_archive_blake3 = "ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2",
    source_archive_bytes = 880640,
    hosting_profile_blake3 = "31c16d1a45192767661610661c3534a17e0f927e887c9edbb24f4d98191e4d82",
    license = "MIT OR Apache-2.0",
    identity_revision = "7ce7e08059bc233ac12407a112e2dd9791d93190",
    delegates = ["did:key:z6MksnXbFoE8zkCkGWhHc8zuxpnEUhrJHv2KECRV4GSv9gkx"],
    threshold = 1,
    author_sigrefs = "848a208c2f8353426fa23e50ab06d98fc64a797e",
    signed_refs_feature = "parent",
    branches = {
      main = "36984e367eafd44c22d2b4f4f47121deb51eece3",
      candidate = "36984e367eafd44c22d2b4f4f47121deb51eece3",
    },
    native_acquisition = {
      seed = "z6Mkmqogy2qEM2ummccUthFEaaHvyYmYBYh3dbe9W4ebScxo",
      signed_refs_feature = "parent",
      revision = "36984e367eafd44c22d2b4f4f47121deb51eece3",
      archive_blake3 = "ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2",
      git_fsck = "verified",
    },
    replication = {
      required = 1,
      observed_in_sync = 13,
    },
    https_acquisition = {
      url = "https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git",
      revision = "36984e367eafd44c22d2b4f4f47121deb51eece3",
      archive_blake3 = "ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2",
      git_fsck = "verified",
      unknown_ref = "rejected",
      receive_pack_discovery = "rejected-http-503",
      receive_pack_post = "rejected-http-503",
    },
    non_claims = [
      "publication does not prove indefinite availability",
      "replica agreement does not prove seed trust",
      "source identity does not prove source or CI correctness",
      "the HTTPS route is a read-only projection and has no governance authority",
      "publication does not grant runtime or release authority",
    ],
  },
}
