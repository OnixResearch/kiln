let evidence = { evaluator = "1.17.0", claim_freshness = true } in
if evidence.claim_freshness then error "freshness-overclaim-detected" else "bounded"
