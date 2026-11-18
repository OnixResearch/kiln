# Typed authoring contract for the shared projected-workspace corpus (#82, #78).
# The local baseline harness in `kiln-web-service` is authoritative: it admits
# every projection through Kiln's source-projection admission and refuses a
# malformed corpus before writing anything.
let projection = import "source-projection-contracts.ncl" in
let MaxFiles = 64 in
let MaxProjections = 8 in
let MaxAttempts = 4 in
let MaxTextBytes = 4096 in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Name =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[a-z][a-z0-9-]{0,63}$" value
  )
in
let Path =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)*$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let Text =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value <= MaxTextBytes
  )
in
let Hex64 =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let distinct = fun values =>
  std.array.all (fun value => std.array.length (std.array.filter (fun other => other == value) values) == 1) values
in
let CorpusFile = { path | Path, text | Text, executable | Bool } in
let Dependency = { name | Name, identity | Hex64 } in
{
  # One corpus file, reused by the build-input corpus (#77).
  File = CorpusFile,
  Corpus = {
    schema | Exact "kiln.workspace-corpus.v1",
    files | Array CorpusFile,
    dependencies | Array Dependency,
    projections | Array projection.Definition,
    attempts | Array Name,
  },
  # Rules that span fields: bounded unique files, projections, dependencies and
  # attempts; at least two projections and two attempts, so sharing and
  # isolation are both exercised; and a fact for every declared dependency.
  Consistent =
    std.contract.from_predicate (fun corpus =>
      let names = std.array.map (fun dependency => dependency.name) corpus.dependencies in
      std.array.length corpus.files >= 1
      && std.array.length corpus.files <= MaxFiles
      && std.array.length corpus.projections >= 2
      && std.array.length corpus.projections <= MaxProjections
      && std.array.length corpus.attempts >= 2
      && std.array.length corpus.attempts <= MaxAttempts
      && distinct (std.array.map (fun file => file.path) corpus.files)
      && distinct (std.array.map (fun definition => definition.id) corpus.projections)
      && distinct names
      && distinct corpus.attempts
      && std.array.all
        (fun definition =>
          std.array.all
            (fun declared => std.array.elem declared.name names)
            (if std.record.has_field "dependencies" definition then definition.dependencies else [])
        )
        corpus.projections
    ),
  # Two projects that share a protocol directory and its declared dependency,
  # an executable script, equal-byte license files at two origins, an excluded
  # environment file and a file neither project selects.
  example = {
    schema = "kiln.workspace-corpus.v1",
    files = [
      { path = "projects/alpha/Cargo.toml", text = "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n", executable = false },
      { path = "projects/alpha/src/lib.rs", text = "pub fn alpha() -> &'static str {\n    \"alpha\"\n}\n", executable = false },
      { path = "projects/alpha/run.sh", text = "#!/bin/sh\nexec cargo test --offline\n", executable = true },
      { path = "projects/alpha/LICENSE", text = "Licensed under MIT OR Apache-2.0.\n", executable = false },
      { path = "projects/alpha/.env", text = "TOKEN=never-materialized\n", executable = false },
      { path = "projects/beta/Cargo.toml", text = "[package]\nname = \"beta\"\nversion = \"0.1.0\"\n", executable = false },
      { path = "projects/beta/src/lib.rs", text = "pub fn beta() -> &'static str {\n    \"beta\"\n}\n", executable = false },
      { path = "projects/beta/LICENSE", text = "Licensed under MIT OR Apache-2.0.\n", executable = false },
      { path = "shared/proto/schema.proto", text = "syntax = \"proto3\";\nmessage Ping {}\n", executable = false },
      { path = "shared/proto/VERSION", text = "1\n", executable = false },
      { path = "docs/README.md", text = "# Corpus\n\nSelected by neither project.\n", executable = false },
    ],
    dependencies = [
      { name = "shared-proto", identity = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1" },
    ],
    projections = [
      {
        schema = "kiln.source-projection.definition.v1",
        id = "alpha-build",
        profile = 'runnable,
        selection = {
          root = "",
          "include" = ["projects/alpha/**", "shared/proto/**"],
          exclude = ["projects/alpha/.env"],
        },
        mounts = [
          { source_prefix = "projects/alpha", destination_prefix = "alpha" },
          { source_prefix = "shared/proto", destination_prefix = "shared/proto" },
        ],
        dependencies = [{ name = "shared-proto", required = true }],
        creation_destinations = [],
        required_capabilities = ['exact_tree, 'executable_bit],
        generated_inputs = 'refused,
        target = { case_sensitive = true, max_path_bytes = 4096, symlinks = true, executable_bit = true },
      },
      {
        schema = "kiln.source-projection.definition.v1",
        id = "beta-build",
        profile = 'runnable,
        selection = {
          root = "",
          "include" = ["projects/beta/**", "shared/proto/**"],
          exclude = [],
        },
        mounts = [
          { source_prefix = "projects/beta", destination_prefix = "beta" },
          { source_prefix = "shared/proto", destination_prefix = "shared/proto" },
        ],
        dependencies = [{ name = "shared-proto", required = true }],
        creation_destinations = [],
        required_capabilities = ['exact_tree],
        generated_inputs = 'refused,
        target = { case_sensitive = true, max_path_bytes = 4096, symlinks = true, executable_bit = true },
      },
    ],
    attempts = ["attempt-one", "attempt-two"],
  },
}
