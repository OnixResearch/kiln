# r[impl kiln.preview.exact] r[impl kiln.preview.runtime]
# Human-authored, opt-in exact runtime selection. This exports no credentials,
# source checkout, derivation/build input, branch or package resolver request.
let Integer = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && std.number.floor value == value
  )
in
let Blake3 =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let StorePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[A-Za-z0-9+._?=-]+$" value
    && !(std.string.is_match "\\.drv$" value)
  )
in
let Executable =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}-[A-Za-z0-9+._?=-]+/bin/[A-Za-z0-9+._-]+$" value
  )
in
let Key =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 256
    && std.string.is_match "^[A-Za-z0-9._-]+:[A-Za-z0-9+/]{43}=$" value
  )
in
let Keys =
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= 8
  )
in
let Argument =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value <= 256
  )
in
let Arguments =
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= 16
  )
in
fun input =>
  let selection =
    input
      | {
        root_path | StorePath,
        executable | Executable,
        arguments | Array Argument | Arguments | default = [],
        trust_identity | Blake3,
        public_keys | Array Key | Keys,
        limits
          | {
            runtime_bytes | Integer 1 134217728 | default = 16777216,
            run_seconds | Integer 1 180 | default = 120,
            output_bytes | Integer 1 65536 | default = 16384,
            expires_at | Integer 1 9007199254740991,
            retain_after_session | std.contract.from_predicate (fun value => value == false) | default = false,
          },
      }
  in selection
