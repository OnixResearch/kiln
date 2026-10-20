let attested_cohort = "1.16.0" in
if attested_cohort != "1.17.0" then error "stale-cohort-detected" else "cohort-ok"
