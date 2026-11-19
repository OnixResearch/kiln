let H = import "../helpers.ncl" in
H.policy [{ rule_id = "raw-path", scope = "filesystem", operation = "read", match_mode = "prefix", subject_pattern = "/home/user", priority = H.rule_priority, decision = "allow" }] "deny" H.identity.generation H.identity.agentos_revision H.non_claims
