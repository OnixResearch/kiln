use super::{AuthorityCurrentness, BranchAuthorityRequest};

const REQUEST_CONTEXT: &str = "onixresearch.basalt.world-branch-authority.request.identity.v1";
const AUTHORITY_CONTEXT: &str =
    "onixresearch.basalt.world-branch-authority.currentness.identity.v1";
const RULE_CONTEXT: &str = "onixresearch.basalt.world-branch-authority.rule.identity.v1";
const RECEIPT_CONTEXT: &str = "onixresearch.basalt.world-branch-authority.receipt.identity.v1";

struct Framer {
    hasher: blake3::Hasher,
}

impl Framer {
    fn new(context: &'static str) -> Self {
        Self {
            hasher: blake3::Hasher::new_derive_key(context),
        }
    }

    fn text(&mut self, value: &str) {
        let length = u64::try_from(value.len()).unwrap_or(u64::MAX);
        self.hasher.update(&length.to_be_bytes());
        self.hasher.update(value.as_bytes());
    }

    fn boolean(&mut self, value: bool) {
        self.hasher.update(&[u8::from(value)]);
    }

    fn finish(self) -> String {
        format!("blake3:{}", self.hasher.finalize().to_hex())
    }
}

pub(super) fn request_identity(request: &BranchAuthorityRequest) -> String {
    let mut frame = Framer::new(REQUEST_CONTEXT);
    frame.text(request.capability_class.as_str());
    frame.text(request.action.as_str());
    frame.text(&request.source_branch_ref);
    frame.text(&request.destination_branch_ref);
    frame.text(&request.capability_ref);
    frame.text(request.source_scope.resource());
    for ability in request.source_scope.abilities() {
        frame.text(ability);
    }
    frame.text(request.destination_scope.resource());
    for ability in request.destination_scope.abilities() {
        frame.text(ability);
    }
    frame.text(&request.policy_generation.to_string());
    frame.boolean(request.mapping_lossless);
    frame.finish()
}

pub(super) fn authority_identity(authority: &AuthorityCurrentness) -> String {
    let mut frame = Framer::new(AUTHORITY_CONTEXT);
    frame.text(&authority.observation_ref);
    frame.boolean(authority.policy.is_current());
    frame.boolean(authority.capability.is_current());
    frame.boolean(authority.revocation.is_current());
    frame.boolean(authority.replay.is_current());
    frame.boolean(authority.scope.is_current());
    frame.boolean(authority.ucan.is_verified());
    frame.finish()
}

pub(super) fn rule_identity(policy_ref: &str, rule_id: &str) -> String {
    let mut frame = Framer::new(RULE_CONTEXT);
    frame.text(policy_ref);
    frame.text(rule_id);
    frame.finish()
}

pub(super) fn receipt_identity(canonical_json: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new_derive_key(RECEIPT_CONTEXT);
    let length = u64::try_from(canonical_json.len()).unwrap_or(u64::MAX);
    hasher.update(&length.to_be_bytes());
    hasher.update(canonical_json);
    format!("blake3:{}", hasher.finalize().to_hex())
}
