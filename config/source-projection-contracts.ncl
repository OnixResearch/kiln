# Typed authoring contract for source projection definitions. Rust admission
# (`kiln_core::projection::admit_definition`) is authoritative and rechecks
# every rule; Nickel counts characters where Rust bounds bytes.
let MaxIdBytes = 128 in
let MaxPatternBytes = 256 in
let MaxPatterns = 32 in
let MaxMounts = 32 in
let MaxDependencies = 32 in
let MaxCreationDestinations = 16 in
let MaxTargetPathBytes = 4096 in
let has_control = fun value => std.string.is_match "[\\x00-\\x1f\\x7f]" value in
let Text = fun max =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length (std.string.trim value) > 0
    && std.string.length value <= max
    && !(has_control value)
  )
in
let canonical = fun value =>
  std.is_string value
  && std.string.length value > 0
  && !(std.string.contains "\\" value)
  && !(has_control value)
  && std.array.all (fun part => part != "" && part != "." && part != "..") (std.string.split "/" value)
in
let literal = fun value => canonical value && !(std.string.is_match "[*?\\[\\]{}]" value) in
let Pattern =
  std.contract.from_predicate (fun value =>
    canonical value
    && std.string.length value <= MaxPatternBytes
    && !(std.string.is_match "[\\[\\]{}]" value)
    && std.array.all (fun part => !(std.string.contains "**" part) || part == "**") (std.string.split "/" value)
  )
in
let Prefix =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && (value == "" || (literal value && std.string.length value <= MaxPatternBytes))
  )
in
let Destination =
  std.contract.from_predicate (fun value =>
    literal value && std.string.length value <= MaxTargetPathBytes
  )
in
let AtMost = fun max =>
  std.contract.from_predicate (fun value => std.is_array value && std.array.length value <= max)
in
let Mount = { source_prefix | Prefix, destination_prefix | Prefix } in
let Dependency = { name | Text MaxIdBytes, required | Bool } in
let Target = {
  case_sensitive | Bool,
  max_path_bytes
    | std.contract.from_predicate (fun value =>
      std.is_number value
      && std.number.is_integer value
      && value >= 1
      && value <= MaxTargetPathBytes
    ),
  symlinks | Bool,
  executable_bit | Bool,
}
in
let Fields = {
  schema
    | std.contract.from_predicate (fun value => value == "kiln.source-projection.definition.v1"),
  id | Text MaxIdBytes,
  profile | [| 'navigation, 'runnable, 'writable |],
  selection
    | {
      root | Prefix,
      "include" | Array Pattern | AtMost MaxPatterns,
      exclude | Array Pattern | AtMost MaxPatterns,
    },
  mounts
    | Array Mount
    | std.contract.from_predicate (fun value =>
      std.array.length value >= 1 && std.array.length value <= MaxMounts
    ),
  dependencies | Array Dependency | AtMost MaxDependencies,
  creation_destinations | Array Destination | AtMost MaxCreationDestinations,
  required_capabilities | Array [| 'exact_tree, 'executable_bit, 'symlinks |],
  generated_inputs | [| 'refused, 'admitted |],
  target | Target,
}
in
# Profile rules: creation destinations are writable-only, runnable and writable
# projections need an exact tree, admitted generated inputs are runnable-only,
# and the target must represent every required capability.
let ProfileRules =
  std.contract.from_predicate (fun definition =>
    (definition.profile == 'writable || definition.creation_destinations == [])
    && (
      definition.profile == 'navigation
      || std.array.elem 'exact_tree definition.required_capabilities
    )
    && (definition.generated_inputs == 'refused || definition.profile == 'runnable)
    && (definition.target.symlinks || !(std.array.elem 'symlinks definition.required_capabilities))
    && (
      definition.target.executable_bit
      || !(std.array.elem 'executable_bit definition.required_capabilities)
    )
  )
in
{
  Definition = std.contract.Sequence [Fields, ProfileRules],
  # A case-sensitive POSIX target with symbolic links and executable bits.
  posix_target = {
    case_sensitive = true,
    max_path_bytes = MaxTargetPathBytes,
    symlinks = true,
    executable_bit = true,
  },
  # A runnable kiln-core projection: the crate mounted at `kiln-core`, its
  # manifest aliased beside the workspace manifest dependency.
  example = {
    schema = "kiln.source-projection.definition.v1",
    id = "kiln-core-build",
    profile = 'runnable,
    selection = {
      root = "crates/kiln-core",
      "include" = ["**"],
      exclude = ["target/**"],
    },
    mounts = [
      { source_prefix = "crates/kiln-core", destination_prefix = "kiln-core" },
      {
        source_prefix = "crates/kiln-core/Cargo.toml",
        destination_prefix = "manifests/kiln-core.toml",
      },
    ],
    dependencies = [{ name = "workspace-manifest", required = true }],
    creation_destinations = [],
    required_capabilities = ['exact_tree, 'executable_bit],
    generated_inputs = 'refused,
    target = posix_target,
  },
}
