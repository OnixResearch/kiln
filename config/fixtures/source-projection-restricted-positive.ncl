# Exported to fixtures/source-projection/restricted/corpus.json for the Rust
# disclosure and sharing tests, and reused by the source-view routes, the
# projected-workspace authority and Lattice scoped context.
#
# One repository snapshot with three views over it: a maintainer with full
# source, a project reader and a scoped agent. It holds a hidden dependency
# (`crates/shared`), equal-byte files at two origins (`crates/app/src/util.rs`
# and `crates/secret/src/key.rs`), a display alias of the shared library
# (`aliases/shared.rs`), a sibling whose name extends a granted one
# (`crates/apple`) and a hidden Cairn prerequisite (`secret-rotation`).
let contracts = import "../source-projection-contracts.ncl" in
let everything = [
  'discover,
  'read_content,
  'read_history,
  'prepare_workspace,
  'modify_source,
  'create_source,
  'delegate,
  'retrieve_result
]
in
let subtree = fun parts => { components = parts, subtree = true } in
let identity_mount = { source_prefix = "", destination_prefix = "" } in
let shared_alias = {
  source_prefix = "crates/shared/src/lib.rs",
  destination_prefix = "aliases/shared.rs",
}
in
let view_definition = fun view_id view_profile patterns view_mounts needs_shared capabilities =>
  {
    schema = "kiln.source-projection.definition.v1",
    id = view_id,
    profile = view_profile,
    selection = { root = "", "include" = patterns, exclude = [] },
    mounts = view_mounts,
    dependencies = [{ name = "shared-crate", required = needs_shared }],
    creation_destinations = [],
    required_capabilities = capabilities,
    generated_inputs = 'refused,
    target = contracts.posix_target,
  }
in
let share_request = fun to scopes abilities not_after child_depth =>
  {
    recipient = to,
    paths = scopes,
    operations = abilities,
    snapshot = { rule = 'exact, revision = contracts.revision },
    not_after_s = not_after,
    depth = child_depth,
  }
in
let reviewer = "did:key:z6MkReviewer" in
{
  schema = "kiln.source-projection.restricted-corpus.v1",
  repository = {
    handle = "restricted-corpus",
    provider = "git-source-v1",
    native = "OnixResearch/restricted-corpus",
  },
  revision = contracts.revision,
  now_s = 1000,
  entries = [
    { path = ".cairn/changes/app-feature/tasks.md", blob = "app-tasks", kind = 'file },
    { path = ".cairn/changes/secret-rotation/tasks.md", blob = "secret-tasks", kind = 'file },
    { path = "crates/app/Cargo.toml", blob = "app-manifest", kind = 'file },
    { path = "crates/app/src/main.rs", blob = "app-main", kind = 'file },
    { path = "crates/app/src/util.rs", blob = "equal-bytes", kind = 'file },
    { path = "crates/apple/README.md", blob = "apple-readme", kind = 'file },
    { path = "crates/secret/Cargo.toml", blob = "secret-manifest", kind = 'file },
    { path = "crates/secret/src/key.rs", blob = "equal-bytes", kind = 'file },
    { path = "crates/shared/src/lib.rs", blob = "shared-lib", kind = 'file },
    { path = "docs/guide.md", blob = "guide", kind = 'file },
  ],
  dependencies = [{ name = "shared-crate", identity_label = "shared-crate-v1" }],
  cairn = [
    {
      change = "app-feature",
      kind = 'task,
      native_id = "app-feature.task.1",
      path = ".cairn/changes/app-feature/tasks.md",
      text = "Wire the app entry point",
      status = 'open,
      prerequisites = ["secret-rotation.task.1"],
    },
    {
      change = "app-feature",
      kind = 'requirement,
      native_id = "app-feature.req.1",
      path = ".cairn/changes/app-feature/tasks.md",
      text = "The app MUST start",
      status = 'done,
      prerequisites = [],
    },
    {
      change = "secret-rotation",
      kind = 'task,
      native_id = "secret-rotation.task.1",
      path = ".cairn/changes/secret-rotation/tasks.md",
      text = "Rotate the signing key",
      status = 'open,
      prerequisites = [],
    },
    {
      change = "secret-rotation",
      kind = 'task,
      native_id = "secret-rotation.task.2",
      path = ".cairn/changes/secret-rotation/tasks.md",
      text = "Audit key consumers",
      status = 'done,
      prerequisites = [],
    },
  ],
  views = [
    {
      name = "maintainer",
      definition =
        view_definition
          "corpus-all"
          'writable
          ["**"]
          [identity_mount]
          true
          ['exact_tree, 'executable_bit],
      holder = {
        audience = "did:key:z6MkMaintainer",
        grants = [{ path = subtree [], operations = everything }],
        not_after_s = 2000,
        depth = 2,
      },
      expect = {
        visible = [
          ".cairn/changes/app-feature/tasks.md",
          ".cairn/changes/secret-rotation/tasks.md",
          "crates/app/Cargo.toml",
          "crates/app/src/main.rs",
          "crates/app/src/util.rs",
          "crates/apple/README.md",
          "crates/secret/Cargo.toml",
          "crates/secret/src/key.rs",
          "crates/shared/src/lib.rs",
          "docs/guide.md",
        ],
        cairn = [
          "app-feature.req.1",
          "app-feature.task.1",
          "secret-rotation.task.1",
          "secret-rotation.task.2",
        ],
        hidden_prerequisites = 0,
      },
    },
    {
      name = "project-reader",
      definition =
        view_definition
          "corpus-app-read"
          'navigation
          ["crates/**", ".cairn/changes/**"]
          [identity_mount, shared_alias]
          false
          [],
      holder = {
        audience = "did:key:z6MkReader",
        grants = [
          { path = subtree ["crates", "app"], operations = ['discover, 'read_content, 'read_history] },
          { path = subtree ["crates", "shared"], operations = ['discover, 'read_content, 'read_history] },
          { path = subtree [".cairn", "changes", "app-feature"], operations = ['discover, 'read_content] },
        ],
        not_after_s = 2000,
        depth = 0,
      },
      expect = {
        visible = [
          ".cairn/changes/app-feature/tasks.md",
          "aliases/shared.rs",
          "crates/app/Cargo.toml",
          "crates/app/src/main.rs",
          "crates/app/src/util.rs",
          "crates/shared/src/lib.rs",
        ],
        cairn = ["app-feature.req.1", "app-feature.task.1"],
        hidden_prerequisites = 1,
      },
    },
    {
      name = "agent",
      definition =
        view_definition
          "corpus-app-agent"
          'writable
          ["crates/app/**", "crates/apple/**", "crates/shared/**", ".cairn/changes/**"]
          [identity_mount, shared_alias]
          true
          ['exact_tree],
      holder = {
        audience = "did:key:z6MkAgent",
        grants = [
          {
            path = subtree ["crates", "app"],
            operations = ['discover, 'read_content, 'prepare_workspace, 'modify_source, 'create_source],
          },
          { path = subtree [".cairn", "changes", "app-feature"], operations = ['discover] },
        ],
        not_after_s = 2000,
        depth = 0,
      },
      expect = {
        visible = [
          ".cairn/changes/app-feature/tasks.md",
          "crates/app/Cargo.toml",
          "crates/app/src/main.rs",
          "crates/app/src/util.rs",
        ],
        cairn = ["app-feature.req.1", "app-feature.task.1"],
        hidden_prerequisites = 1,
      },
    },
  ],
  shares = [
    {
      name = "attenuated-read",
      view = "project-reader",
      sharer = "maintainer",
      request = share_request reviewer [subtree ["crates", "app"]] ['discover, 'read_content] 1500 0,
      expect = 'admitted,
    },
    {
      name = "read-does-not-imply-share",
      view = "project-reader",
      sharer = "project-reader",
      request = share_request reviewer [subtree ["crates", "app"]] ['discover] 1500 0,
      expect = 'delegation_not_permitted,
    },
    {
      name = "alias-is-not-a-resource",
      view = "project-reader",
      sharer = "maintainer",
      request = share_request reviewer [{ components = ["aliases", "shared.rs"], subtree = false }] ['discover] 1500 0,
      expect = 'widen_resource,
    },
    {
      name = "outside-the-view-ceiling",
      view = "project-reader",
      sharer = "maintainer",
      request = share_request reviewer [subtree ["docs"]] ['discover] 1500 0,
      expect = 'widen_resource,
    },
    {
      name = "navigation-view-cannot-share-edit",
      view = "project-reader",
      sharer = "maintainer",
      request = share_request reviewer [subtree ["crates", "app"]] ['modify_source] 1500 0,
      expect = 'widen_operation,
    },
    {
      name = "outlives-the-sharer",
      view = "project-reader",
      sharer = "maintainer",
      request = share_request reviewer [subtree ["crates", "app"]] ['discover] 2500 0,
      expect = 'outlives_parent,
    },
    {
      name = "depth-not-below-the-sharer",
      view = "project-reader",
      sharer = "maintainer",
      request = share_request reviewer [subtree ["crates", "app"]] ['discover, 'delegate] 1500 2,
      expect = 'depth_exceeded,
    },
    {
      name = "tag-cannot-widen-the-snapshot",
      view = "project-reader",
      sharer = "maintainer",
      request =
        (share_request reviewer [subtree ["crates", "app"]] ['discover] 1500 0)
        & { snapshot | force = { rule = 'ref, name = "refs/tags/v2", evolution = 'fast_forward } },
      expect = 'widen_snapshot,
    },
  ],
}
  | contracts.Corpus
