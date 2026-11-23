# r[impl kiln.implementation.authority.config]
# Typed builder for one `kiln.implementation-authority-config.v1` selection.
# Export to the JSON file passed as `kiln-implementer --authority-config FILE`;
# `ImplementationAuthorityConfig::validate` re-checks every bound below.
#
# Profiles:
# - 'absent: every protected operation denies.
# - { local_operator = { operator } }: explicit compatibility; never admits
#   projection routes.
# - { verified_ucan = { ... } }: Basalt packaged or BLAKE3-pinned policy, static
#   Ed25519 key resolver, bounded proof directory, a revocation feed no older
#   than `max_age_ms`, replay records retained `retention_ms`, and three
#   distinct holder seeds (coordinator, verifier, publisher).
#
# Every record is closed. Exporting grants nothing: the seeds, proofs, policy
# and revocation feed are read by Kiln at selection.
let MAX_TRUSTED_ROOTS = 16 in
let MAX_STATIC_KEYS = 64 in
let MAX_AUTHORITY_PATH_BYTES = 4096 in
let MAX_AUTHORITY_NAME_BYTES = 256 in
let MAX_U64 = 18446744073709551615 in
let SCHEMA = "kiln.implementation-authority-config.v1" in
let unique = fun keys =>
  std.array.length (
    std.array.fold_left (fun seen k => if std.array.elem k seen then seen else seen @ [k]) [] keys
  ) == std.array.length keys
in
# Non-empty, trimmed, printable ASCII; ASCII length is the byte length.
let is_name = fun value =>
  std.is_string value
  && std.string.length value <= MAX_AUTHORITY_NAME_BYTES
  && std.string.is_match "^[!-~]([ -~]*[!-~])?$" value
in
# UTF-8 width: one byte per scalar plus one per threshold it crosses.
let utf8_bytes = fun value =>
  let count = fun pattern => std.array.length (std.string.find_all pattern value) in
  count "(?s)."
  + count "[\\x{80}-\\x{10FFFF}]"
  + count "[\\x{800}-\\x{10FFFF}]"
  + count "[\\x{10000}-\\x{10FFFF}]"
in
let Name = std.contract.from_predicate is_name in
let DidKey =
  std.contract.from_predicate (fun value =>
    is_name value && std.string.is_match "^did:key:z" value
  )
in
let Hex32 =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/" value
    && !std.string.is_match "\\p{Cc}" value
    && std.array.all (fun segment => segment != "..") (std.string.split "/" value)
    && utf8_bytes value <= MAX_AUTHORITY_PATH_BYTES
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0 && value <= MAX_U64 && value == std.number.truncate value
  )
in
let SecretRef = { seed_path | AbsolutePath } in
let Key = { did | DidKey, ed25519 | Hex32 } in
let Policy =
  std.contract.custom (fun label value =>
    if std.is_enum value && value == 'packaged then
      'Ok value
    else if std.is_record value && std.record.fields value == ["file"] then
      'Ok (std.contract.apply { file | { path | AbsolutePath, blake3 | Hex32 } } label value)
    else
      'Error { message = "policy must be 'packaged or { file = { path, blake3 } }" }
  )
in
let VerifiedUcan = {
  audience | Name,
  generation | Positive,
  policy | Policy,
  trusted_roots
    | Array DidKey
    | std.contract.from_predicate (fun roots =>
      std.array.length roots >= 1
      && std.array.length roots <= MAX_TRUSTED_ROOTS
      && unique roots
    ),
  keys
    | Array Key
    | std.contract.from_predicate (fun keys =>
      std.array.length keys >= 1
      && std.array.length keys <= MAX_STATIC_KEYS
      && unique (std.array.map (fun key => key.did) keys)
    ),
  proofs_dir | AbsolutePath,
  revocation | { path | AbsolutePath, max_age_ms | Positive },
  replay | { retention_ms | Positive },
  holders
    | { coordinator | SecretRef, verifier | SecretRef, publisher | SecretRef }
    | std.contract.from_predicate (fun holders =>
      unique [
        holders.coordinator.seed_path,
        holders.verifier.seed_path,
        holders.publisher.seed_path
      ]
    ),
  projection_enabled | Bool,
}
in
let Profile =
  std.contract.custom (fun label value =>
    if std.is_enum value && value == 'absent then
      'Ok value
    else if std.is_record value && std.record.fields value == ["local_operator"] then
      'Ok (std.contract.apply { local_operator | { operator | Name } } label value)
    else if std.is_record value && std.record.fields value == ["verified_ucan"] then
      'Ok (std.contract.apply { verified_ucan | VerifiedUcan } label value)
    else
      'Error { message = "profile must be 'absent, { local_operator } or { verified_ucan }" }
  )
in
fun input =>
  input
    | {
      schema | std.contract.from_predicate (fun value => value == SCHEMA),
      profile | Profile,
    }
