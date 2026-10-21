# One reviewed exact-source shell operation. No ambient source keys or credentials.
let NonEmpty = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
let Positive = std.contract.from_predicate (fun value => std.is_number value && value > 0 && std.number.floor value == value) in
fun inputs =>
  let checked | { name | NonEmpty, command | NonEmpty, timeout_seconds | Positive, created | NonEmpty } = inputs in
  {
    meta = {
      name = checked.name,
      description = "Operator-authorized exact GitHub source build and core behavior probe",
      created = checked.created,
      modified = checked.created,
    },
    nodes = [{
      id = 0, label = "exact-source-build-test", x = 0, y = 0,
      input_ports = [{ id = 0, label = "stdin", type_tag = "string" }],
      output_ports = [
        { id = 1, label = "stdout", type_tag = "string" },
        { id = 2, label = "stderr", type_tag = "string" },
        { id = 3, label = "exit", type_tag = "exitcode" },
      ],
    }],
    edges = [],
    configs = [{
      node_id = 0, kind = "shell", command = checked.command,
      cwd = [], env = [], secret_env = [],
      http_method = "", url = "", headers = [], body = [],
      file_operation = "", file_path = "", file_content = [], overwrite = false,
      allowed_roots = [], timeout_secs = checked.timeout_seconds, pipe_stdin = false,
    }],
    triggers = [],
  }
