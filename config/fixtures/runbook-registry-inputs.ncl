{
  runbooks = [
    {
      id = "security-audit",
      display_name = "Security Audit",
      description = "Run read-only audit checks against an admitted target.",
      mode = "read_only",
      lattice = {
        bundle_id = "security-audit",
        bundle_identity = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        workflow_name = "security-audit",
        workflow_revision = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        nodes = [
          {
            id = "audit",
            kind = "test",
            effect_class = "test",
            adapter_kind = "local",
            operation_identity = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
          },
        ],
        edges = [],
      },
      params = [
        {
          name = "profile",
          type = { kind = "enum", values = ["fast", "full"] },
          required = false,
          default = "fast",
          bindings = [
            { node = "audit", shell_env = "LATTICE_PARAM_PROFILE" },
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
          project_id = "kiln-audit",
          references = ["refs/heads/main"],
          allow_commit_sha = false,
        },
      ],
    },
    {
      id = "flake-update",
      display_name = "Flake Update",
      description = "Update flake inputs and validate the build graph.",
      mode = "mutating",
      lattice = {
        bundle_id = "flake-update",
        bundle_identity = "1111111111111111111111111111111111111111111111111111111111111111",
        workflow_name = "flake-update",
        workflow_revision = "2222222222222222222222222222222222222222222222222222222222222222",
        nodes = [
          {
            id = "update",
            kind = "build",
            effect_class = "build",
            adapter_kind = "local",
            operation_identity = "3333333333333333333333333333333333333333333333333333333333333333",
          },
          {
            id = "verify",
            kind = "test",
            effect_class = "test",
            adapter_kind = "local",
            operation_identity = "4444444444444444444444444444444444444444444444444444444444444444",
          },
        ],
        edges = [
          { source = "update", target = "verify" },
        ],
      },
      params = [
        {
          name = "max_steps",
          type = { kind = "integer", min = 1, max = 20 },
          required = false,
          default = 5,
          bindings = [
            { node = "update", shell_env = "LATTICE_PARAM_MAX_STEPS" },
          ],
        },
        {
          name = "manifest_path",
          type = { kind = "relative_path", max_bytes = 128 },
          required = true,
          bindings = [
            { node = "update", shell_env = "LATTICE_PARAM_MANIFEST_PATH" },
            { node = "verify", shell_env = "LATTICE_PARAM_MANIFEST_PATH" },
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
    },
  ],
  grants = [
    {
      principal = "local:operator",
      runbooks = ["flake-update"],
      not_after_ms = 1900000000000,
    },
  ],
}
