const REQUEST_IDENTITY_DOMAIN: &str = "cairn.gate-request.identity.v1";
const RESULT_IDENTITY_DOMAIN: &str = "cairn.gate-result.identity.v1";

struct Frame(blake3::Hasher);

impl Frame {
    fn new(domain: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain.as_bytes());
        Self(hasher)
    }

    fn bytes(&mut self, label: &str, value: &[u8]) {
        self.length_prefixed(label.as_bytes());
        self.length_prefixed(value);
    }

    fn string(&mut self, label: &str, value: &str) {
        self.bytes(label, value.as_bytes());
    }

    fn boolean(&mut self, label: &str, value: bool) {
        self.bytes(label, &[u8::from(value)]);
    }

    #[cfg_attr(
        dylint_lib = "octet",
        allow(
            missing_const_fn,
            reason = "Cairn gate-contract owner: frame writes call the non-const BLAKE3 hasher."
        )
    )]
    fn count(&mut self, label: &str, value: usize) {
        match u64::try_from(value) {
            Ok(value) => self.bytes(label, &value.to_le_bytes()),
            Err(_) => self.string(label, "unrepresentable-count"),
        }
    }

    #[cfg_attr(
        dylint_lib = "octet",
        allow(
            missing_const_fn,
            reason = "Cairn gate-contract owner: blake3::Hasher::update cannot run in const evaluation."
        )
    )]
    fn length_prefixed(&mut self, bytes: &[u8]) {
        match u64::try_from(bytes.len()) {
            Ok(length) => {
                self.0.update(&length.to_le_bytes());
                self.0.update(bytes);
            }
            Err(_) => {
                self.0.update(&0_u64.to_le_bytes());
                self.0.update(b"unrepresentable-length");
            }
        }
    }

    fn finish(self) -> crate::Identity {
        crate::Identity::from_digest(self.0.finalize())
    }
}

#[must_use]
pub fn request_identity(request: &crate::GateRequest) -> crate::Identity {
    let mut frame = Frame::new(REQUEST_IDENTITY_DOMAIN);
    frame.string("schema", &request.schema);
    frame.string("idempotency_key", request.idempotency_key.as_str());
    frame_applicability(&mut frame, &request.applicability);
    frame.string("target_change", request.target.change.as_str());
    frame.string("target_stage", request.target.stage.as_str());
    frame_tool(&mut frame, &request.tool);
    frame.finish()
}

#[must_use]
pub fn result_identity(result: &crate::GateResult) -> crate::Identity {
    let mut frame = Frame::new(RESULT_IDENTITY_DOMAIN);
    frame.string("schema", &result.schema);
    frame.string("request_identity", result.request_identity.as_str());
    frame_applicability(&mut frame, &result.applicability);
    frame_tool(&mut frame, &result.tool);
    frame_observation(&mut frame, &result.observation);
    frame.count("non_claim_count", result.non_claims.len());
    for non_claim in &result.non_claims {
        frame_text(&mut frame, "non_claim", non_claim);
    }
    frame.finish()
}

fn frame_applicability(frame: &mut Frame, value: &crate::GateApplicability) {
    frame.string("candidate_identity", value.candidate_identity.as_str());
    frame.string(
        "candidate_root_identity",
        value.candidate_root_identity.as_str(),
    );
    frame.string(
        "pipeline_revision_identity",
        value.pipeline_revision_identity.as_str(),
    );
    frame.string("policy_identity", value.policy_identity.as_str());
    frame_evidence(frame, &value.evidence_identities);
    frame.string(
        "provider_profile_identity",
        value.provider_profile_identity.as_str(),
    );
    frame.string("effect_identity", value.effect_identity.as_str());
}

fn frame_evidence(frame: &mut Frame, identities: &[crate::Identity]) {
    frame.count("evidence_count", identities.len());
    for identity in identities {
        frame.string("evidence_identity", identity.as_str());
    }
}

fn frame_tool(frame: &mut Frame, value: &crate::ToolCohort) {
    frame.string("tool_source_revision", value.source_revision.as_str());
    frame.string(
        "tool_executable_identity",
        value.executable_identity.as_str(),
    );
    frame.string("tool_machine_schema", &value.machine_schema);
    frame.string("tool_diagnostic_schema", &value.diagnostic_schema);
    frame.string("tool_policy_schema", &value.policy_schema);
    frame.string("tool_gate_receipt_schema", &value.gate_receipt_schema);
}

fn frame_observation(frame: &mut Frame, value: &crate::NativeGateObservation) {
    frame.string("observation_schema", &value.schema);
    frame.string("observation_change", value.change.as_str());
    frame.string("observation_stage", value.stage.as_str());
    frame.string("observation_verdict", value.verdict.as_str());
    frame.boolean("observation_valid", value.valid);
    frame.string("native_input_identity", value.input_identity.as_str());
    frame.string("native_policy_identity", value.policy_identity.as_str());
    frame.string("native_receipt_identity", value.receipt_identity.as_str());
    frame.count("finding_count", value.findings.len());
    for finding in &value.findings {
        frame_finding(frame, finding);
    }
}

fn frame_finding(frame: &mut Frame, value: &crate::GateFinding) {
    frame.string("finding_code", value.code.as_str());
    frame_text(frame, "finding_message", &value.message);
    frame.string("finding_scope", value.scope.as_str());
    frame.string("finding_promoted", value.promoted.as_str());
}

fn frame_text(frame: &mut Frame, label: &str, value: &crate::BoundedText) {
    frame.string(label, value.as_str());
}
