let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Hash = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == 64
) in
let Envelope = {
  type_name | NonEmpty,
  schema_line | NonEmpty,
  mode | NonEmpty,
} in
{
  upstream = {
    revision = "837d730799faffe6a088cfd6dc49e08c7e0f1822",
    bundle_path = "fixtures/radicle-cob-interoperability/external-cairn-cob.bundle",
    bundle_blake3 | Hash = "8274f326f3165f7b19ee69aaa0dcd7a9db02c75fc0c63ed3609150676ab3396d",
    provenance_path = "fixtures/radicle-cob-interoperability/provenance.json",
    provenance_blake3 | Hash = "3e7a0eaa0a5aa3c7688116bb34dbdd464b982006efb30e5478cff263db23a76b",
  },
  consumer = {
    authority = "read_only",
    formats | Array Envelope = [
      { type_name = "xyz.onixresearch.cairn-change", schema_line = "cairn.change.v1", mode = "external-fixture" },
      { type_name = "xyz.onixresearch.rfd", schema_line = "cairn.rfd.v1", mode = "profile-envelope" },
      { type_name = "xyz.onixresearch.adr", schema_line = "cairn.adr.v1", mode = "profile-envelope" },
      { type_name = "xyz.onixresearch.verification-obligation", schema_line = "cairn.verification-obligation.v1", mode = "profile-envelope" },
      { type_name = "xyz.onixresearch.release-decision", schema_line = "cairn.release-decision.v1", mode = "profile-envelope" },
      { type_name = "xyz.onixresearch.policy-exception", schema_line = "cairn.policy-exception.v1", mode = "profile-envelope" },
    ],
    non_claims = [
      "only the Cairn change profile has an upstream bundle at the pinned revision",
      "profile envelopes do not claim provider reduction equivalence",
      "fixture identity does not grant mutation or release authority",
    ],
  },
}
