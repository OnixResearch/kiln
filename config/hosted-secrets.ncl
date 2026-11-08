# Host-only policy. Export into ServiceProfile.secrets, never accept from customer RPC.
# Supply an existing owned mode0600 raw32 Ed25519 seed file. No key is generated.
# Values remain in the selected SecretSpec provider; SQLite stores references only.
let c = import "github-contracts.ncl" in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let Audience =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 512
    && std.string.is_match "^[!-~]+$" value
  )
in
let BoundedArray =
  std.contract.from_predicate (fun values =>
    std.is_array values && std.array.length values <= 128
  )
in
let NonEmptyAtMost = fun maximum =>
  std.contract.from_predicate (fun values =>
    std.is_array values && std.array.length values >= 1 && std.array.length values <= maximum
  )
in
let UniqueStrings =
  std.contract.from_predicate (fun values =>
    let check = fun state value =>
      if std.array.elem value state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [value], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
  )
in
let UniqueResolverIds =
  std.contract.from_predicate (fun resolvers =>
    let check = fun state resolver =>
      if std.array.elem resolver.id state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [resolver.id], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } resolvers).duplicate
  )
in
fun input =>
  input
    | {
      issuer
        | std.contract.from_predicate (fun value =>
          std.is_string value
          && std.string.length value <= 512
          && std.string.is_match "^https://[^ @?#]+[^/ @?#]$" value
        ),
      signing_key_path | c.AbsolutePath,
      audiences | Array Audience | NonEmptyAtMost 128 | UniqueStrings,
      token_lifetime_seconds
        | std.contract.from_predicate (fun value =>
          std.is_number value
          && value == std.number.floor value
          && value >= 1
          && value <= 300
        ),
      resolvers
        | Array {
          id | Identifier,
          organization | Identifier,
          project | Identifier,
          reference | c.SecretReference,
        }
        | BoundedArray
        | UniqueResolverIds,
    }
