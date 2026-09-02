let Upstream = import "upstream-default.ncl" in
let KilnTraceabilityProfile = {
  id = "cairn-default",
  requirement_sources = [
    { root = ".cairn/specs", source_kind = "accepted_specs", extensions = ["md"] },
  ],
  evidence_sources = [
    { root = "crates", source_kind = "rust_source", extensions = ["rs"] },
    { root = "crates", source_kind = "nickel_source", extensions = ["ncl"] },
    { root = "tools", source_kind = "tool_source", extensions = ["rs", "ncl"] },
    { root = "src", source_kind = "tool_source", extensions = ["mjs"] },
    { root = "tests", source_kind = "tool_source", extensions = ["mjs", "sh"] },
  ],
  marker_verbs = ["impl", "verify", "depends", "related"],
  assurance_level = "declared",
  anchor_kind = "exact_marker_bytes",
} in
let TrellisTraceabilityProfile = {
  id = "trellis-rust",
  requirement_sources = [
    { root = "docs/requirements.md", source_kind = "requirements_doc", extensions = ["md"] },
  ],
  evidence_sources = [
    { root = "src", source_kind = "rust_source", extensions = ["rs"] },
    { root = "tests", source_kind = "rust_source", extensions = ["rs"] },
  ],
  marker_verbs = ["impl", "verify", "depends", "related"],
  assurance_level = "declared",
  anchor_kind = "exact_marker_bytes",
} in
Upstream & {
  name | force = "kiln-default",
  policy_schema_compatibility | force = Upstream.policy_schema_compatibility & {
    refresh_command | force = "nix run path:$CAIRN_SOURCE#cairn -- policy export --source cairn-policy/default.ncl --output cairn-policy/generated/cairn-policy.json",
  },
  traceability_policy | force = {
    default_profile = "cairn-default",
    profiles = [KilnTraceabilityProfile, TrellisTraceabilityProfile],
  },
}
