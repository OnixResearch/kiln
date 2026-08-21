# Versioned producer contract for Cairn gate requests and bound results.

let MaximumEvidenceIdentities = 256 in
let MaximumTextBytes = 4096 in

let is_identity = fun value =>
  std.is_string value && std.string.is_match "^b3:[0-9a-f]{64}$" value
in

let Identity = std.contract.from_predicate is_identity in

let Label = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= 128
  && std.string.is_match "^[a-z][a-z0-9._:-]*$" value
) in

let BoundedText = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumTextBytes
  && std.string.is_match "^[^\\x00-\\x1f\\x7f]+$" value
) in

let GitRevision = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{40}$" value
) in

let CanonicalEvidenceIdentities = std.contract.from_predicate (fun value =>
  std.is_array value
  && std.array.length value <= MaximumEvidenceIdentities
  && std.array.all is_identity value
  && std.array.sort std.string.compare value == value
  && std.array.all
    (fun item =>
      std.array.fold_left
        (fun count candidate => if candidate == item then count + 1 else count)
        0
        value
      == 1
    )
    value
) in

let GateStage = std.contract.from_predicate (fun value =>
  value == "proposal" || value == "design" || value == "tasks"
) in

let GateVerdict = std.contract.from_predicate (fun value =>
  value == "pass" || value == "block"
) in

let DiagnosticSeverity = std.contract.from_predicate (fun value =>
  value == "error" || value == "warning" || value == "info"
) in

let Nullable = fun Contract =>
  std.contract.custom (fun label value =>
    if value == null then 'Ok value else std.contract.check Contract label value
  )
in

let ToolCohort = {
  source_revision | GitRevision,
  executable_identity | Identity,
  machine_schema | std.contract.from_predicate (fun value => value == "cairn.cli-machine.v1"),
  diagnostic_schema | std.contract.from_predicate (fun value => value == "cairn.cli-machine-diagnostic.v1"),
  policy_schema | std.contract.from_predicate (fun value => value == "cairn.policy-schema.v1"),
  gate_receipt_schema | std.contract.from_predicate (fun value => value == "cairn.gate-receipt.v1"),
} in

let GateApplicability = {
  candidate_identity | Identity,
  candidate_root_identity | Identity,
  pipeline_revision_identity | Identity,
  policy_identity | Identity,
  evidence_identities | CanonicalEvidenceIdentities,
  provider_profile_identity | Identity,
  effect_identity | Identity,
} in

let ChangeGateTarget = {
  change | Label,
  stage | GateStage,
} in

let GateFinding = {
  code | Label,
  message | BoundedText,
  scope | Label,
  promoted | Label,
} in

let NativeGateObservation = {
  schema | std.contract.from_predicate (fun value => value == "cairn.native-gate-observation.v1"),
  change | Label,
  stage | GateStage,
  verdict | GateVerdict,
  valid | Bool,
  input_identity | Identity,
  policy_identity | Identity,
  receipt_identity | Identity,
  findings | Array GateFinding,
} in

let MachineDiagnostic = {
  schema | std.contract.from_predicate (fun value => value == "cairn.cli-machine-diagnostic.v1"),
  code | Label,
  severity | DiagnosticSeverity,
  message | BoundedText,
  target | BoundedText,
  fix | Nullable BoundedText,
} in

let GateRequest = {
  schema | std.contract.from_predicate (fun value => value == "cairn.gate-request.v1"),
  identity | Identity,
  idempotency_key | Identity,
  applicability | GateApplicability,
  target | ChangeGateTarget,
  tool | ToolCohort,
} in

let GateResult = {
  schema | std.contract.from_predicate (fun value => value == "cairn.gate-result.v1"),
  identity | Identity,
  request_identity | Identity,
  applicability | GateApplicability,
  tool | ToolCohort,
  observation | NativeGateObservation,
  non_claims | Array BoundedText,
} in

{
  identity = Identity,
  label = Label,
  bounded_text = BoundedText,
  git_revision = GitRevision,
  tool_cohort = ToolCohort,
  gate_applicability = GateApplicability,
  change_gate_target = ChangeGateTarget,
  gate_finding = GateFinding,
  native_gate_observation = NativeGateObservation,
  machine_diagnostic = MachineDiagnostic,
  gate_request = GateRequest,
  gate_result = GateResult,
}
