# Kiln-owned Lattice worker guest workflow: three ordered shell steps that
# write, checksum and verify a journal in the attempt's own writable workspace.
# The journal includes the guest's per-attempt machine identity, so two
# attempts on the same immutable image produce different workspace contents.

let Port = { id | Number, label | String, type_tag | String } in
let Node = { id | Number, label | String, x | Number, y | Number, input_ports | Array Port, output_ports | Array Port } in
let Edge = { source | Number, target | Number } in
let EnvVar = { key | String, value | String } in
let SecretEnvVar = { name | String, provider | String, key | String, field | Array String } in
let NodeConfig = { node_id | Number, kind | String, command | String, cwd | Array String, env | Array EnvVar, secret_env | Array SecretEnvVar, http_method | String, url | String, headers | Array EnvVar, body | Array String, file_operation | String, file_path | String, file_content | Array String, overwrite | Bool, allowed_roots | Array String, timeout_secs | Number, pipe_stdin | Bool } in
let Meta = { name | String, description | String, created | String, modified | String } in
let Workflow = { meta | Meta, nodes | Array Node, edges | Array Edge, configs | Array NodeConfig } in
let shell_timeout_secs = 60 in
let shell_defaults = {
  kind = "shell",
  cwd = [],
  env = [],
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
  timeout_secs = shell_timeout_secs,
  pipe_stdin = true,
} in
let shell_config = fun config_node_id config_command => shell_defaults & { node_id = config_node_id, command = config_command } in
let node = fun node_id node_label node_x => {
  id = node_id,
  label = node_label,
  x = node_x,
  y = 0,
  input_ports = [{ id = node_id * 10, label = "stdin", type_tag = "string" }],
  output_ports = [
    { id = node_id * 10 + 1, label = "stdout", type_tag = "string" },
    { id = node_id * 10 + 2, label = "stderr", type_tag = "string" },
    { id = node_id * 10 + 3, label = "exit", type_tag = "exitcode" },
  ],
} in
(
{
  meta = {
    name = "workspace-journal",
    description = "Write, checksum and verify a journal in the attempt workspace",
    created = "2026-11-19T00:00:00Z",
    modified = "2026-11-19T00:00:00Z",
  },
  nodes = [
    node 1 "write journal" 0,
    node 2 "checksum journal" 28,
    node 3 "verify journal" 56,
  ],
  edges = [
    { source = 11, target = 20 },
    { source = 21, target = 30 },
  ],
  configs = [
    shell_config 1 "mkdir -p /workspace/checkout/out && { printf 'kiln lattice worker journal\\n'; cat /etc/machine-id; } > /workspace/checkout/out/journal.txt && echo written",
    shell_config 2 "cd /workspace/checkout/out && sha256sum journal.txt > journal.sha256 && cat journal.sha256",
    shell_config 3 "cd /workspace/checkout/out && sha256sum -c journal.sha256",
  ],
}
) | Workflow
