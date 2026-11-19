let H = import "../helpers.ncl" in
H.policy [{ rule_id = "unknown", scope = "unknown", operation = "read", match_mode = "exact", subject_pattern = "unknown://x", priority = H.rule_priority, decision = "allow" }] "deny" H.identity.generation H.identity.agentos_revision H.non_claims
