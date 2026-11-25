# Seed RunbookRegistry (`kiln.runbook-registry.v1`) for the 4 bundles under
# `runbooks/packs/kiln-seed-runbooks/workflow-templates/`.
# Export via `nickel export --format json runbooks/registry.seed.ncl`.
(import "../config/runbook-registry.ncl") {
  runbooks = [
    {
      id = "flake-update",
      display_name = "Flake Update",
      description = "Update flake inputs, run verification checks, and pause at an approval gate before Kiln publication.",
      mode = "mutating",
      approval_gate = "kiln_publication",
      lattice = {
        bundle_id = "flake-update",
        bundle_identity = "28cf2c469fe40f1eb231baef3ff86aad5ba4b890cea6415fee54aaeb8b8e75eb",
        workflow_name = "flake-update",
        workflow_revision = "5bcd6a32798819bd3186f408d76932ef4f79ed6fbb29e0d485f1c1ea93791401",
        nodes = [
          {
            id = "update-flake",
            kind = "build",
            effect_class = "build",
            adapter_kind = "local",
            operation_identity = "c282c8d89783c97744bb2ef24a163a6b19aca015042a23b33be83d2e1b2e73f8",
          },
          {
            id = "verify-flake",
            kind = "test",
            effect_class = "test",
            adapter_kind = "local",
            operation_identity = "c3fc0eaea96fd679f457c6c89950b0d9c24441b43213be75d4da9ed8357735ce",
          },
        ],
        edges = [
          { source = "update-flake", target = "verify-flake" },
        ],
      },
      params = [
        {
          name = "flake_dir",
          type = { kind = "relative_path", max_bytes = 128 },
          required = false,
          default = ".",
          bindings = [
            { node = "update-flake", shell_env = "LATTICE_PARAM_FLAKE_DIR" },
          ],
        },
        {
          name = "update_scope",
          type = { kind = "enum", values = ["all", "nixpkgs", "cairn", "toolchain"] },
          required = false,
          default = "all",
          bindings = [
            { node = "update-flake", shell_env = "LATTICE_PARAM_UPDATE_SCOPE" },
          ],
        },
        {
          name = "run_check",
          type = { kind = "bool" },
          required = false,
          default = true,
          bindings = [
            { node = "update-flake", shell_env = "LATTICE_PARAM_RUN_CHECK" },
          ],
        },
      ],
      allowed_targets = [
        {
          repository = "onixresearch/kiln",
          project_id = "kiln-flake-update",
          references = ["refs/heads/main"],
          allow_commit_sha = true,
        },
      ],
      fanout = {
        enabled = true,
        default_max_concurrency = 1,
        max_concurrency_limit = 4,
        default_budget_cap = 8,
        max_budget_cap = 32,
        max_targets = 16,
        write_scopes = ["flake.lock"],
      },
    },
    {
      id = "security-audit",
      display_name = "Security Audit",
      description = "Run Cairn, cargo-audit, and Octet checks and retain a Provenance summary receipt.",
      mode = "read_only",
      approval_gate = "none",
      lattice = {
        bundle_id = "security-audit",
        bundle_identity = "86790db3549cf9ed5646e21546fd031929e687be690af967927df3fe65ceb1ba",
        workflow_name = "security-audit",
        workflow_revision = "2cc6f8223cf41a03ea0affa8b2b141d9f03814371d73e2242e2c8ba8f1ef1842",
        nodes = [
          {
            id = "audit",
            kind = "test",
            effect_class = "test",
            adapter_kind = "local",
            operation_identity = "0c7a51d71e36c886a3afa6d01877536646af25051d7ba2d4dcc2a8c1e8df040a",
          },
        ],
        edges = [],
      },
      params = [
        {
          name = "crate_root",
          type = { kind = "relative_path", max_bytes = 128 },
          required = false,
          default = ".",
          bindings = [
            { node = "audit", shell_env = "LATTICE_PARAM_CRATE_ROOT" },
          ],
        },
        {
          name = "audit_scope",
          type = { kind = "enum", values = ["all", "cairn", "cargo-audit", "octet"] },
          required = false,
          default = "all",
          bindings = [
            { node = "audit", shell_env = "LATTICE_PARAM_AUDIT_SCOPE" },
          ],
        },
        {
          name = "strict",
          type = { kind = "bool" },
          required = false,
          default = true,
          bindings = [
            { node = "audit", shell_env = "LATTICE_PARAM_STRICT" },
          ],
        },
      ],
      allowed_targets = [
        {
          repository = "onixresearch/kiln",
          project_id = "kiln-security-audit",
          references = ["refs/heads/main"],
          allow_commit_sha = false,
        },
      ],
      fanout = {
        enabled = true,
        default_max_concurrency = 2,
        max_concurrency_limit = 8,
        default_budget_cap = 16,
        max_budget_cap = 64,
        max_targets = 32,
        write_scopes = [],
      },
    },
    {
      id = "license-ci-scaffold",
      display_name = "License & CI Scaffold",
      description = "Scaffold LICENSE and GitHub CI workflow files from pinned templates and pause at an approval gate before PR publication.",
      mode = "mutating",
      approval_gate = "kiln_publication",
      lattice = {
        bundle_id = "license-ci-scaffold",
        bundle_identity = "2b77bd7e6b76a848d181befb75664496ef349014e75647bbbd16f300d5e44c5e",
        workflow_name = "license-ci-scaffold",
        workflow_revision = "d61e05e5ad8d240a1a3ee267480ea2a176030903cb663ad653e442c4f39a583a",
        nodes = [
          {
            id = "scaffold",
            kind = "build",
            effect_class = "build",
            adapter_kind = "local",
            operation_identity = "7d3a118489ad63d1ab82b10e472994e05994f44da8a603cb6657d7823c133604",
          },
          {
            id = "verify-scaffold",
            kind = "test",
            effect_class = "test",
            adapter_kind = "local",
            operation_identity = "1ff2372e88ad772f890b6e9f583bc5d052e9e76da2f3b95236aec0a8ec262554",
          },
        ],
        edges = [
          { source = "scaffold", target = "verify-scaffold" },
        ],
      },
      params = [
        {
          name = "license_spdx",
          type = { kind = "enum", values = ["apache-2.0", "mit", "mit-or-apache-2.0"] },
          required = false,
          default = "mit-or-apache-2.0",
          bindings = [
            { node = "scaffold", shell_env = "LATTICE_PARAM_LICENSE_SPDX" },
          ],
        },
        {
          name = "ci_template",
          type = { kind = "enum", values = ["rust-cargo", "nix-flake", "rust-and-nix"] },
          required = false,
          default = "rust-and-nix",
          bindings = [
            { node = "scaffold", shell_env = "LATTICE_PARAM_CI_TEMPLATE" },
          ],
        },
        {
          name = "overwrite_existing",
          type = { kind = "bool" },
          required = false,
          default = false,
          bindings = [
            { node = "scaffold", shell_env = "LATTICE_PARAM_OVERWRITE_EXISTING" },
          ],
        },
      ],
      allowed_targets = [
        {
          repository = "onixresearch/kiln",
          project_id = "kiln-license-ci-scaffold",
          references = ["refs/heads/main"],
          allow_commit_sha = true,
        },
      ],
      fanout = {
        enabled = true,
        default_max_concurrency = 1,
        max_concurrency_limit = 4,
        default_budget_cap = 8,
        max_budget_cap = 32,
        max_targets = 16,
        write_scopes = ["LICENSE", ".github/workflows/ci.yml"],
      },
    },
    {
      id = "release-cut",
      display_name = "Release Cut",
      description = "Prepare a verified release candidate and require an approval gate before tag creation or release publication.",
      mode = "mutating",
      approval_gate = "kiln_publication",
      lattice = {
        bundle_id = "release-cut",
        bundle_identity = "7b0cb77a7ef9ab08366a95b6e63bfdeb669f56e4416497ba88b03e8f121878ac",
        workflow_name = "release-cut",
        workflow_revision = "d8709011726f79344aa198bdd5cf01e3530797b4a975a28bb8cba434eb7b3dd5",
        nodes = [
          {
            id = "prepare-release",
            kind = "build",
            effect_class = "build",
            adapter_kind = "local",
            operation_identity = "6e48823d8aa6dcaff583655da87f6c91d758f891279065665c1ed9dd588be9fa",
          },
          {
            id = "verify-release",
            kind = "test",
            effect_class = "test",
            adapter_kind = "local",
            operation_identity = "ca024fde4b9ca7bde6ded1079b30d10ab15d48cf740a6debb19c9bdf3f3b5ef4",
          },
        ],
        edges = [
          { source = "prepare-release", target = "verify-release" },
        ],
      },
      params = [
        {
          name = "crate_manifest",
          type = { kind = "relative_path", max_bytes = 128 },
          required = false,
          default = "Cargo.toml",
          bindings = [
            { node = "prepare-release", shell_env = "LATTICE_PARAM_CRATE_MANIFEST" },
          ],
        },
        {
          name = "release_channel",
          type = { kind = "enum", values = ["alpha", "beta", "rc", "stable"] },
          required = false,
          default = "rc",
          bindings = [
            { node = "prepare-release", shell_env = "LATTICE_PARAM_RELEASE_CHANNEL" },
          ],
        },
        {
          name = "bump_level",
          type = { kind = "enum", values = ["patch", "minor", "major"] },
          required = false,
          default = "patch",
          bindings = [
            { node = "prepare-release", shell_env = "LATTICE_PARAM_BUMP_LEVEL" },
          ],
        },
        {
          name = "allow_dirty",
          type = { kind = "bool" },
          required = false,
          default = false,
          bindings = [
            { node = "prepare-release", shell_env = "LATTICE_PARAM_ALLOW_DIRTY" },
          ],
        },
      ],
      allowed_targets = [
        {
          repository = "onixresearch/kiln",
          project_id = "kiln-release-cut",
          references = ["refs/heads/main"],
          allow_commit_sha = true,
        },
      ],
      fanout = {
        enabled = true,
        default_max_concurrency = 1,
        max_concurrency_limit = 2,
        default_budget_cap = 4,
        max_budget_cap = 16,
        max_targets = 8,
        write_scopes = ["Cargo.toml", "CHANGELOG.md"],
      },
    },
  ],
  grants = [
    {
      principal = "local:operator",
      runbooks = ["flake-update", "license-ci-scaffold", "release-cut"],
      not_after_ms = 2000000000000,
    },
  ],
}
