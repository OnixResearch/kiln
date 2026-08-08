let registry = import "../cob-profiles.ncl" in
registry & {
  profiles = [{
    id = "invalid",
    provider = "radicle",
    type_name = "unknown",
    schema_line = "unknown",
    mapper_version = 1,
    upstream_revision = "unknown",
    reducer = "unknown",
    license = "MIT",
    capabilities = ["mutation"],
    actions = ["update"],
    limits = { max_heads = 0, max_payload_bytes = 0, max_actions = 0, retry_limit = 0 },
    complete_head_resolution = false,
    non_claims = [],
  }],
}
