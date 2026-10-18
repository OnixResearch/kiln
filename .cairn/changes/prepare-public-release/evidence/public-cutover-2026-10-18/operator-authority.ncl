let NonEmpty = std.contract.from_predicate (fun value => std.is_string value && std.string.length value > 0) in
{
  schema = "kiln.public-cutover-authority.v1",
  operator_request | NonEmpty = "do it",
  accepted_plan = [
    "Finalize the separate license/provenance work and select the exact release candidate",
    "Run fresh anonymous acquisition/build checks and candidate-specific privacy, rights, and combined-license review",
    "Promote the approved candidate to main/default, then make the GitHub projection public",
  ],
  repository = "OnixResearch/kiln",
  radicle_rid = "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
  distribution_scope = "source repository and evidence only; no binary release or production deployment",
  authorized = {
    candidate_preparation = true,
    canonical_and_projection_source_push = true,
    main_fast_forward = true,
    github_default_branch_main = true,
    github_public_visibility = true,
  },
  excluded = [
    "Third-party relicensing or extension of owner-specific grants",
    "New dependency revisions or sibling producer publication",
    "Governance, delegate or threshold changes",
    "History rewriting or deletion of unrelated refs",
    "Binary distribution compatibility or production-service approval",
    "Inclusion or deletion of unrelated vendor Cargo.lock changes",
  ],
  rollback = {
    main = "2c529b154d625735c43246cad2178d70813a01c3",
    github_default_branch = "publication-candidate-2026-10-15",
    github_default_revision = "db552f504236b3e330be46618e2fd29fb864f190",
    prior_github_visibility = "private",
    warning = "Changing visibility back cannot retract already copied public data",
  },
  authority_boundary = "The operator request authorizes these repository effects, not copyright title, third-party rights, universal privacy clearance or release eligibility by model assertion",
}
