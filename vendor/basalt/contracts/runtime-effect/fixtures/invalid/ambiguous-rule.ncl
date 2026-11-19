let H = import "../helpers.ncl" in
H.policy [
  { rule_id = "allow", scope = "filesystem", operation = "read", match_mode = "exact", subject_pattern = "cap://workspace/input", priority = H.rule_priority, decision = "allow" },
  { rule_id = "deny", scope = "filesystem", operation = "read", match_mode = "exact", subject_pattern = "cap://workspace/input", priority = H.rule_priority, decision = "deny" },
] "deny" H.identity.generation H.identity.agentos_revision H.non_claims
