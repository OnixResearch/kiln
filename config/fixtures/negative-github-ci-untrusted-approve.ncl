# Standalone GitHub CI has no durable approval surface: it admits only deny.
let c = import "../github-contracts.ncl" in
(import "../../fixtures/github/service-profile.ncl")
& { event_policy = { untrusted_pull_requests = "approve" } }
  | c.ServiceProfile
