let Port = { id | Number, label | String, type_tag | String } in
let Node = { id | Number, label | String, x | Number, y | Number, input_ports | Array Port, output_ports | Array Port } in
let Edge = { source | Number, target | Number } in
let EnvVar = { key | String, value | String } in
let SecretEnvVar = { name | String, provider | String, key | String, field | Array String, value | optional | Dyn } in
let NodeConfig = {
  node_id | Number,
  kind | String,
  runtime_kind | String,
  runtime_key | String,
  effect_policy | String,
  cache_policy | String,
  retry_policy | String,
  artifact_policy | String,
  command | String,
  cwd | Array String,
  env | Array EnvVar,
  secret_env | Array SecretEnvVar,
  http_method | String,
  url | String,
  headers | Array EnvVar,
  body | Array String,
  file_operation | String,
  file_path | String,
  file_content | Array String,
  overwrite | Bool,
  allowed_roots | Array String,
  timeout_secs | Number,
  pipe_stdin | Bool,
}
in
let Meta = { name | String, description | String, created | String, modified | String } in
let Workflow = { meta | Meta, nodes | Array Node, edges | Array Edge, configs | Array NodeConfig } in
({
  meta = {
    name = "security-audit",
    description = "Run read-only Cairn, cargo-audit, and Octet checks and emit a Provenance summary receipt.",
    created = "2026-11-25T00:00:00Z",
    modified = "2026-11-25T00:00:00Z",
  },
  nodes = [
    {
      id = 1,
      label = "audit",
      x = 0,
      y = 0,
      input_ports = [{ id = 10, label = "stdin", type_tag = "string" }],
      output_ports = [
        { id = 11, label = "stdout", type_tag = "string" },
        { id = 12, label = "stderr", type_tag = "string" },
        { id = 13, label = "exit", type_tag = "exitcode" },
      ],
    },
  ],
  edges = [],
  configs = [
    {
      node_id = 1,
      kind = "shell",
      runtime_kind = "shell",
      runtime_key = "default",
      effect_policy = "",
      cache_policy = "",
      retry_policy = "",
      artifact_policy = "",
      command = "python3 assets/run-security-audit.py \"${LATTICE_PARAM_CRATE_ROOT}\" \"${LATTICE_PARAM_AUDIT_SCOPE}\" \"${LATTICE_PARAM_STRICT}\"",
      cwd = ["."],
      env = [
        { key = "LATTICE_PARAM_CRATE_ROOT", value = "." },
        { key = "LATTICE_PARAM_AUDIT_SCOPE", value = "all" },
        { key = "LATTICE_PARAM_STRICT", value = "true" },
      ],
      secret_env = [],
      http_method = "",
      url = "",
      headers = [],
      body = [],
      file_operation = "",
      file_path = "",
      file_content = [],
      overwrite = false,
      allowed_roots = [],
      timeout_secs = 600,
      pipe_stdin = true,
    },
  ],
}
) | Workflow
