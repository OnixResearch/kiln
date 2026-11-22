# Typed authoring contract for build-input requests and the build-input
# qualification corpus (#77). The Rust planner is authoritative: it admits every
# request again and refuses a malformed one before any effect.
let projection = import "source-projection-contracts.ncl" in
let corpus = import "workspace-corpus-contracts.ncl" in
let MaxIdBytes = 128 in
let MaxTargets = 16 in
let MaxArguments = 32 in
let MaxArgumentBytes = 256 in
let MaxVariables = 32 in
let MaxValueBytes = 1024 in
let MaxFiles = 64 in
let MaxProjects = 8 in
let MaxCases = 8 in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let has_control = fun value => std.string.is_match "[\\x00-\\x1f\\x7f]" value in
let Text = fun max =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value >= 1
    && std.string.length value <= max
    && !(has_control value)
  )
in
let Argument =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value <= MaxArgumentBytes && !(has_control value)
  )
in
let Between = fun min max =>
  std.contract.from_predicate (fun values =>
    std.array.length values >= min && std.array.length values <= max
  )
in
let Name =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[a-z][a-z0-9-]{0,63}$" value
  )
in
let distinct = fun values =>
  std.array.all (fun value => std.array.length (std.array.filter (fun other => other == value) values) == 1) values
in
let Environment =
  std.contract.from_predicate (fun environment =>
    std.is_record environment
    && std.array.length (std.record.fields environment) <= MaxVariables
    && std.array.all
      (fun name => std.string.is_match "^[A-Za-z_][A-Za-z0-9_]{0,127}$" name)
      (std.record.fields environment)
    && std.array.all
      (fun value =>
        std.is_string value
        && std.string.length value <= MaxValueBytes
        && !(has_control value)
      )
      (std.record.values environment)
  )
in
let RequestFields = {
  schema | Exact "kiln.build-input.request.v1",
  profile
    | std.contract.from_predicate (fun value =>
      std.array.elem value ["cargo_compatibility.v1", "nix_compatibility.v1"]
    ),
  targets | Array (Text MaxIdBytes) | Between 1 MaxTargets,
  command | Array Argument | Between 1 MaxArguments,
  toolchain | { name | Text MaxIdBytes, version | Text MaxIdBytes },
  environment | Environment,
}
in
# Unique targets and a named program.
let RequestRules =
  std.contract.from_predicate (fun request =>
    distinct request.targets && std.string.length (std.array.first request.command) >= 1
  )
in
let BuildRequest = std.contract.Sequence [RequestFields, RequestRules] in
let Project = {
  id | Name,
  scope
    | {
      root | String,
      "include" | Array String,
      exclude | Array String,
      depends_on | Array String,
      graphs | Array [| 'cargo, 'nix |],
    },
}
in
let Case = { id | Name, definition | projection.Definition, request | BuildRequest } in
let root_files = ["Cargo.toml", "Cargo.lock", ".cargo/**", "rust-toolchain.toml"] in
let view = fun view_id selected placements =>
  {
    schema = "kiln.source-projection.definition.v1",
    id = view_id,
    profile = 'runnable,
    selection = { root = "", "include" = selected, exclude = [] },
    mounts = placements,
    dependencies = [],
    creation_destinations = [],
    required_capabilities = ['exact_tree, 'executable_bit],
    generated_inputs = 'refused,
    target = { case_sensitive = true, max_path_bytes = 4096, symlinks = true, executable_bit = true },
  }
in
let identity = [{ source_prefix = "", destination_prefix = "" }] in
let cargo_test = fun package =>
  {
    schema = "kiln.build-input.request.v1",
    profile = "cargo_compatibility.v1",
    targets = [package],
    command = ["cargo", "test", "--offline", "--locked", "--package", package],
    toolchain = { name = "rust", version = "1.97.1" },
    environment = { CARGO_TERM_COLOR = "never" },
  }
in
let project_scope = fun scope_root =>
  { root = scope_root, "include" = ["**"], exclude = [], depends_on = [], graphs = ['cargo] }
in
{
  Request = BuildRequest,
  Corpus = {
    schema | Exact "kiln.build-input-corpus.v1",
    files | Array corpus.File,
    projects | Array Project,
    cases | Array Case,
    shared_edit | { case | Name, file | corpus.File },
  },
  # Rules that span fields: bounded unique files, projects and cases; each
  # definition carries its case's id; and the shared edit names a case.
  Consistent =
    std.contract.from_predicate (fun value =>
      let case_ids = std.array.map (fun case => case.id) value.cases in
      std.array.length value.files >= 1
      && std.array.length value.files <= MaxFiles
      && std.array.length value.projects >= 1
      && std.array.length value.projects <= MaxProjects
      && std.array.length value.cases >= 1
      && std.array.length value.cases <= MaxCases
      && distinct (std.array.map (fun file => file.path) value.files)
      && distinct (std.array.map (fun project => project.id) value.projects)
      && distinct case_ids
      && std.array.all (fun case => case.definition.id == case.id) value.cases
      && std.array.elem value.shared_edit.case case_ids
    ),
  # A two-project Cargo workspace with glob members, a shared path dependency,
  # inherited workspace fields, a root lockfile, Cargo configuration and a
  # toolchain file. Alpha's build script reads a package asset; beta reads a
  # file of alpha's package that no manifest declares.
  example = {
    schema = "kiln.build-input-corpus.v1",
    files = [
      {
        path = "Cargo.toml",
        text =
          m%"
          [workspace]
          resolver = "2"
          members = ["projects/*", "shared/*"]

          [workspace.package]
          version = "0.1.0"
          edition = "2021"
          publish = false

          [workspace.dependencies]
          proto = { path = "shared/proto" }
        "%
          ++ "\n",
        executable = false,
      },
      {
        path = "Cargo.lock",
        text =
          m%"
          # This file is automatically @generated by Cargo.
          # It is not intended for manual editing.
          version = 4

          [[package]]
          name = "alpha"
          version = "0.1.0"
          dependencies = [
           "proto",
          ]

          [[package]]
          name = "beta"
          version = "0.1.0"
          dependencies = [
           "proto",
          ]

          [[package]]
          name = "proto"
          version = "0.1.0"
        "%
          ++ "\n",
        executable = false,
      },
      {
        path = ".cargo/config.toml",
        text = "[env]\nKILN_BUILD_PROFILE = \"cargo_compatibility.v1\"\n",
        executable = false,
      },
      {
        path = "rust-toolchain.toml",
        text = "[toolchain]\nchannel = \"1.97.1\"\nprofile = \"minimal\"\n",
        executable = false,
      },
      {
        path = "projects/alpha/Cargo.toml",
        text =
          m%"
          [package]
          name = "alpha"
          version.workspace = true
          edition.workspace = true
          publish.workspace = true

          [dependencies]
          proto.workspace = true
        "%
          ++ "\n",
        executable = false,
      },
      {
        path = "projects/alpha/build.rs",
        text =
          m%"
          fn main() {
              println!("cargo:rerun-if-changed=assets/banner.txt");
              let banner = std::fs::read_to_string("assets/banner.txt").expect("the banner is a package asset");
              println!("cargo:rustc-env=ALPHA_BANNER={}", banner.trim());
          }
        "%
          ++ "\n",
        executable = false,
      },
      { path = "projects/alpha/assets/banner.txt", text = "alpha banner\n", executable = false },
      {
        path = "projects/alpha/src/lib.rs",
        text =
          m%"
          /// The shared protocol version plus one.
          pub fn alpha() -> u32 {
              proto::VERSION + 1
          }

          /// The build profile the workspace's Cargo configuration names.
          pub const PROFILE: &str = env!("KILN_BUILD_PROFILE");

          /// The banner the build script read from a package asset.
          pub const BANNER: &str = env!("ALPHA_BANNER");

          #[cfg(test)]
          mod tests {
              #[test]
              fn builds_from_its_declared_inputs() {
                  assert_eq!(super::alpha(), 2);
                  assert_eq!(super::PROFILE, "cargo_compatibility.v1");
                  assert_eq!(super::BANNER, "alpha banner");
              }
          }
        "%
          ++ "\n",
        executable = false,
      },
      {
        path = "projects/beta/Cargo.toml",
        text =
          m%"
          [package]
          name = "beta"
          version.workspace = true
          edition.workspace = true
          publish.workspace = true

          [dependencies]
          proto.workspace = true
        "%
          ++ "\n",
        executable = false,
      },
      {
        path = "projects/beta/src/lib.rs",
        text =
          m%"
          /// The shared protocol version plus two.
          pub fn beta() -> u32 {
              proto::VERSION + 2
          }

          /// A file of another package that no manifest declares.
          pub const BORROWED: &str = include_str!("../../alpha/assets/banner.txt");

          #[cfg(test)]
          mod tests {
              #[test]
              fn reads_across_packages() {
                  assert_eq!(super::beta(), 3);
                  assert!(super::BORROWED.starts_with("alpha"));
              }
          }
        "%
          ++ "\n",
        executable = false,
      },
      {
        path = "shared/proto/Cargo.toml",
        text =
          m%"
          [package]
          name = "proto"
          version.workspace = true
          edition.workspace = true
          publish.workspace = true
        "%
          ++ "\n",
        executable = false,
      },
      {
        path = "shared/proto/src/lib.rs",
        text = "/// The protocol version both projects build against.\npub const VERSION: u32 = 1;\n",
        executable = false,
      },
      {
        path = "docs/README.md",
        text = "# Build-input corpus\n\nNo build view selects this file.\n",
        executable = false,
      },
    ],
    projects = [
      { id = "alpha", scope = project_scope "projects/alpha" },
      { id = "beta", scope = project_scope "projects/beta" },
    ],
    cases = [
      {
        id = "alpha-tests",
        definition = view "alpha-tests" (["projects/alpha/**", "shared/**"] @ root_files) identity,
        request = cargo_test "alpha",
      },
      {
        id = "beta-tests",
        definition = view "beta-tests" (["projects/beta/**", "shared/**"] @ root_files) identity,
        request = cargo_test "beta",
      },
      {
        id = "alpha-without-proto",
        definition = view "alpha-without-proto" (["projects/alpha/**"] @ root_files) identity,
        request = cargo_test "alpha",
      },
      {
        id = "alpha-without-configuration",
        definition =
          view
            "alpha-without-configuration"
            ["projects/alpha/**", "shared/**", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
            identity,
        request = cargo_test "alpha",
      },
      {
        id = "alpha-relocated",
        definition =
          view
            "alpha-relocated"
            (["projects/alpha/**", "shared/**"] @ root_files)
            [
              { source_prefix = "projects/alpha", destination_prefix = "alpha" },
              { source_prefix = "", destination_prefix = "" }
            ],
        request = cargo_test "alpha",
      },
    ],
    shared_edit = {
      case = "alpha-tests",
      file = {
        path = "shared/proto/src/lib.rs",
        text = "/// The protocol version both projects build against.\npub const VERSION: u32 = 2;\n",
        executable = false,
      },
    },
  },
}
