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
# Source-view authority, `kiln.source-authority-coverage.v1`. Rust
# (`kiln_core::projection::{authority,disclosure,sharing}`) is authoritative
# and rechecks every rule. Path scopes are framed per component with an
# explicit subtree marker; reading never implies delegating; a share is
# attenuated against the sharer's current scope and the view's frozen ceiling.
let MaxPathComponents = 256 in
let MaxGrants = 64 in
let MaxSharePaths = 32 in
let MaxDelegationDepth = 8 in
let MaxPrincipalBytes = 512 in
let MaxRefNameBytes = 256 in
let SourceOperation = [|
  'discover,
  'read_content,
  'read_history,
  'prepare_workspace,
  'modify_source,
  'create_source,
  'delegate,
  'retrieve_result
|]
in
let Component =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && value != ""
    && value != "."
    && value != ".."
    && !(std.string.contains "/" value)
    && !(std.string.contains "\\" value)
    && !(std.string.is_match "[*?\\[\\]{}]" value)
    && !(has_control value)
  )
in
let NonEmpty = std.contract.from_predicate (fun value => std.is_array value && value != []) in
# The repository root is the empty component list with `subtree`; an exact
# scope must name a path.
let PathScope =
  std.contract.Sequence [
    { components | Array Component | AtMost MaxPathComponents, subtree | Bool },
    std.contract.from_predicate (fun scope => scope.subtree || scope.components != []),
  ]
in
let Grant = { path | PathScope, operations | Array SourceOperation | NonEmpty } in
# One exact audience DID: no wildcard and no whitespace.
let Principal =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^did:[^\\s*]+$" value
    && std.string.length value <= MaxPrincipalBytes
  )
in
let Depth =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && std.number.is_integer value
    && value >= 0
    && value <= MaxDelegationDepth
  )
in
let ExternalRef = {
  contract_version | std.number.PosNat,
  provider_namespace | Text MaxIdBytes,
  native_kind | Text MaxIdBytes,
  native_id | Text MaxIdBytes,
}
in
let Snapshot =
  std.contract.any_of [
    { rule | [| 'exact |], revision | ExternalRef },
    {
      rule | [| 'ref |],
      name | Text MaxRefNameBytes,
      evolution | [| 'fast_forward, 'rewrite |],
    },
  ]
in
# A delegating child must keep depth to delegate with.
let ShareRequest =
  std.contract.Sequence [
    {
      recipient | Principal,
      paths | Array PathScope | NonEmpty | AtMost MaxSharePaths,
      operations | Array SourceOperation | NonEmpty,
      snapshot | Snapshot,
      not_after_s | std.number.PosNat,
      depth | Depth,
    },
    std.contract.from_predicate (fun share =>
      share.depth >= 1 || !(std.array.elem 'delegate share.operations)
    ),
  ]
in
let Holder = {
  audience | Principal,
  grants | Array Grant | NonEmpty | AtMost MaxGrants,
  not_after_s | std.number.PosNat,
  depth | Depth,
}
in
let ShareOutcome = [|
  'admitted,
  'request_invalid,
  'parent_expired,
  'delegation_not_permitted,
  'widen_resource,
  'widen_operation,
  'widen_snapshot,
  'outlives_parent,
  'depth_exceeded
|]
in
let example_revision = {
  contract_version = 1,
  provider_namespace = "git",
  native_kind = "commit",
  native_id = "1111111111111111111111111111111111111111",
}
in
# The shared restricted-view corpus: one exact snapshot, the views over it
# with each holder's grants and expected disclosure, and share cases.
let CairnItem = {
  change | Text MaxIdBytes,
  kind | [| 'change, 'requirement, 'task, 'decision, 'evidence |],
  native_id | Text MaxIdBytes,
  path | Destination,
  text | String,
  status | [| 'open, 'blocked, 'done |],
  prerequisites | Array (Text MaxIdBytes),
}
in
let RestrictedCorpus = {
  schema
    | std.contract.from_predicate (fun value =>
      value == "kiln.source-projection.restricted-corpus.v1"
    ),
  repository | { handle | Text MaxIdBytes, provider | Text MaxIdBytes, native | Text MaxIdBytes },
  revision | ExternalRef,
  now_s | std.number.PosNat,
  entries
    | Array {
      path | Destination,
      blob | Text MaxIdBytes,
      kind | [| 'file, 'executable, 'symlink |],
    },
  dependencies | Array { name | Text MaxIdBytes, identity_label | Text MaxIdBytes },
  cairn | Array CairnItem,
  views
    | Array {
      name | Text MaxIdBytes,
      definition | std.contract.Sequence [Fields, ProfileRules],
      holder | Holder,
      expect
        | {
          visible | Array Destination,
          cairn | Array (Text MaxIdBytes),
          hidden_prerequisites | std.number.Nat,
        },
    },
  shares
    | Array {
      name | Text MaxIdBytes,
      view | Text MaxIdBytes,
      sharer | Text MaxIdBytes,
      request | ShareRequest,
      expect | ShareOutcome,
    },
}
in
{
  Definition = std.contract.Sequence [Fields, ProfileRules],
  Scope = PathScope,
  SourceGrant = Grant,
  Share = ShareRequest,
  Corpus = RestrictedCorpus,
  revision = example_revision,
  # A reader sharing read access to one crate for a bounded time.
  share_example = {
    recipient = "did:key:z6MkReviewer",
    paths = [{ components = ["crates", "app"], subtree = true }],
    operations = ['discover, 'read_content],
    snapshot = { rule = 'exact, revision = example_revision },
    not_after_s = 1500,
    depth = 0,
  },
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
