let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let Profile = {
  id | NonEmpty,
  provider | NonEmpty,
  type_name | NonEmpty,
  schema_line | NonEmpty,
  mapper_version | Positive,
  upstream_revision | NonEmpty,
  reducer | NonEmpty,
  license | NonEmpty,
  capabilities | Array NonEmpty,
  actions | Array NonEmpty,
  limits | {
    max_heads | Positive,
    max_payload_bytes | Positive,
    max_actions | Positive,
    retry_limit | Positive,
  },
  complete_head_resolution | Bool,
  non_claims | Array NonEmpty,
} in
let MaxHeads = 32 in
let MaxPayloadBytes = 1048576 in
let MaxActions = 16 in
let RetryLimit = 3 in
let ReadOnly = ["typed_read"] in
let CairnRevision = "837d730799faffe6a088cfd6dc49e08c7e0f1822" in
let CairnReducer = "radicle-cob-0.20.0" in
let RadicleRevision = "radicle-0.24.0" in
let BrokerRevision = "a688afde1f730ef60402a809d9cdadc55e8dd749" in
let Limits = {
  max_heads = MaxHeads,
  max_payload_bytes = MaxPayloadBytes,
  max_actions = MaxActions,
  retry_limit = RetryLimit,
} in
let NonClaims = [
  "local reduction does not prove remote synchronization",
  "a signature observation does not prove actor trust or action authority",
  "a COB projection does not replace the Kiln journal or release authority",
] in
{
  profiles | Array Profile = [
    { id = "radicle-issue-v1", provider = "radicle", type_name = "xyz.radicle.issue", schema_line = "radicle.issue.v1", mapper_version = 1, upstream_revision = RadicleRevision, reducer = "radicle-issue-0.24.0", license = "MIT OR Apache-2.0", capabilities = ["typed_read", "event_intake"], actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "radicle-patch-v1", provider = "radicle", type_name = "xyz.radicle.patch", schema_line = "radicle.patch.v1", mapper_version = 1, upstream_revision = RadicleRevision, reducer = "radicle-patch-0.24.0", license = "MIT OR Apache-2.0", capabilities = ["typed_read", "event_intake"], actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "radicle-ci-job-v1", provider = "radicle-ci-broker", type_name = "xyz.radicle.ci-job", schema_line = "radicle.ci-job.v1", mapper_version = 1, upstream_revision = BrokerRevision, reducer = "radicle-ci-job-v1", license = "MIT OR Apache-2.0", capabilities = ["typed_read", "status_projection"], actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "cairn-change-v1", provider = "cairn-cob", type_name = "xyz.onixresearch.cairn-change", schema_line = "cairn.change.v1", mapper_version = 1, upstream_revision = CairnRevision, reducer = CairnReducer, license = "MIT OR Apache-2.0", capabilities = ReadOnly, actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "cairn-rfd-v1", provider = "cairn-cob", type_name = "xyz.onixresearch.rfd", schema_line = "cairn.rfd.v1", mapper_version = 1, upstream_revision = CairnRevision, reducer = CairnReducer, license = "MIT OR Apache-2.0", capabilities = ReadOnly, actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "cairn-adr-v1", provider = "cairn-cob", type_name = "xyz.onixresearch.adr", schema_line = "cairn.adr.v1", mapper_version = 1, upstream_revision = CairnRevision, reducer = CairnReducer, license = "MIT OR Apache-2.0", capabilities = ReadOnly, actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "cairn-verification-obligation-v1", provider = "cairn-cob", type_name = "xyz.onixresearch.verification-obligation", schema_line = "cairn.verification-obligation.v1", mapper_version = 1, upstream_revision = CairnRevision, reducer = CairnReducer, license = "MIT OR Apache-2.0", capabilities = ReadOnly, actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "cairn-release-decision-v1", provider = "cairn-cob", type_name = "xyz.onixresearch.release-decision", schema_line = "cairn.release-decision.v1", mapper_version = 1, upstream_revision = CairnRevision, reducer = CairnReducer, license = "MIT OR Apache-2.0", capabilities = ReadOnly, actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
    { id = "cairn-policy-exception-v1", provider = "cairn-cob", type_name = "xyz.onixresearch.policy-exception", schema_line = "cairn.policy-exception.v1", mapper_version = 1, upstream_revision = CairnRevision, reducer = CairnReducer, license = "MIT OR Apache-2.0", capabilities = ReadOnly, actions = [], limits = Limits, complete_head_resolution = true, non_claims = NonClaims },
  ],
}
