let H = import "../helpers.ncl" in
H.policy [{ rule_id = "unknown", scope = "filesystem", operation = "execute", match_mode = "exact", subject_pattern = "cap://workspace/input", priority = H.rule_priority, decision = "allow" }] "deny" H.identity.generation H.identity.agentos_revision H.non_claims
