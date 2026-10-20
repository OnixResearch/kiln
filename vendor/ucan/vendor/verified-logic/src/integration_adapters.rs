//! User-facing deterministic adapters over existing verified Trellis primitives.
//!
//! These adapters are intentionally thin. They translate domain-shaped finite
//! inputs into existing Trellis admission helpers and return explicit accept /
//! reject metadata. They do not claim full OpenRaft, UCAN, Nix, Nickel, storage,
//! or crash-recovery conformance.

use crate::nickel_record_merge::ExecField;
use crate::nix_closure::ClosureItem;
use crate::nix_eval_closure_policy::NixEvalClosurePolicyOutcome;
use crate::nix_eval_closure_policy::nix_eval_closure_policy;
use crate::raft_consensus_core;
use crate::raft_consensus_core::RaftReadLinearizerMetadata;
use crate::raft_consensus_core::raft_openraft_read_linearizer_admission_result;
use crate::ucan_token::UcanPayload;
use crate::ucan_token::validate_envelope;
use crate::wal_recovery::can_append;
use crate::wal_recovery::needs_recovery;
use crate::wal_recovery::replay_count;
use crate::wal_recovery::replay_range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationAdapterDomain {
    ConsensusRead,
    UcanAuthorization,
    NixClosurePolicy,
    StorageWalRecovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationAdapterRejection {
    ApplyCatchupRequired,
    InvalidUcanEnvelope,
    NixPolicyRejected,
    WalAppendWouldOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegrationAdapterDecision {
    pub domain: IntegrationAdapterDomain,
    pub accepted: bool,
    pub rejection: Option<IntegrationAdapterRejection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsensusReadAdapterInput {
    pub read_log_index: u64,
    pub last_applied_index: u64,
}

pub struct UcanAuthorizationAdapterInput<'a> {
    pub payload: &'a UcanPayload,
    pub signature_valid: bool,
    pub now: u64,
    pub tolerance: u64,
}

pub struct NixClosurePolicyAdapterInput<'a> {
    pub base_attrs: &'a Vec<(Vec<u8>, u64)>,
    pub overlay_attrs: &'a Vec<(Vec<u8>, u64)>,
    pub required_closure_keys: &'a Vec<u64>,
    pub dependency_batches: &'a Vec<Vec<u64>>,
    pub raw_name: &'a Vec<u8>,
    pub substring_start: usize,
    pub substring_len: usize,
    pub replace_from: &'a Vec<Vec<u8>>,
    pub replace_to: &'a Vec<Vec<u8>>,
    pub version: &'a Vec<u8>,
    pub minimum_version: &'a Vec<u8>,
    pub start: &'a Vec<ClosureItem>,
    pub operator: &'a Vec<(u64, Vec<ClosureItem>)>,
    pub left_record: &'a Vec<ExecField>,
    pub right_record: &'a Vec<ExecField>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageWalRecoveryAdapterInput {
    pub checkpoint_lsn: u64,
    pub wal_end: u64,
    pub append_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageWalRecoveryAdapterDecision {
    pub decision: IntegrationAdapterDecision,
    pub replay_start: u64,
    pub replay_end: u64,
    pub replay_count: u64,
    pub needs_recovery: bool,
}

fn accepted(domain: IntegrationAdapterDomain) -> IntegrationAdapterDecision {
    IntegrationAdapterDecision {
        domain,
        accepted: true,
        rejection: None,
    }
}

fn rejected(domain: IntegrationAdapterDomain, rejection: IntegrationAdapterRejection) -> IntegrationAdapterDecision {
    IntegrationAdapterDecision {
        domain,
        accepted: false,
        rejection: Some(rejection),
    }
}

fn consensus_read_adapter_core(input: ConsensusReadAdapterInput) -> IntegrationAdapterDecision {
    let meta = RaftReadLinearizerMetadata {
        read_log_index: input.read_log_index,
        last_applied_index: input.last_applied_index,
    };
    let result = raft_openraft_read_linearizer_admission_result(&meta);
    if result.admitted {
        accepted(IntegrationAdapterDomain::ConsensusRead)
    } else {
        debug_assert_eq!(result.failing_check, raft_consensus_core::RAFT_OPENRAFT_APPLY_CATCHUP_REQUIRED);
        rejected(IntegrationAdapterDomain::ConsensusRead, IntegrationAdapterRejection::ApplyCatchupRequired)
    }
}

// r[impl integration-adapters.user-facing-adapters]
pub fn consensus_read_adapter(input: ConsensusReadAdapterInput) -> IntegrationAdapterDecision {
    consensus_read_adapter_core(input)
}

fn ucan_authorization_adapter_core(input: UcanAuthorizationAdapterInput<'_>) -> IntegrationAdapterDecision {
    match validate_envelope(input.payload, input.signature_valid, input.now, input.tolerance) {
        Ok(true) => accepted(IntegrationAdapterDomain::UcanAuthorization),
        Ok(false) | Err(_) => {
            rejected(IntegrationAdapterDomain::UcanAuthorization, IntegrationAdapterRejection::InvalidUcanEnvelope)
        }
    }
}

// r[impl integration-adapters.user-facing-adapters]
pub fn ucan_authorization_adapter(input: UcanAuthorizationAdapterInput<'_>) -> IntegrationAdapterDecision {
    ucan_authorization_adapter_core(input)
}

fn nix_closure_policy_adapter_core(input: NixClosurePolicyAdapterInput<'_>) -> IntegrationAdapterDecision {
    match nix_eval_closure_policy(
        input.base_attrs,
        input.overlay_attrs,
        input.required_closure_keys,
        input.dependency_batches,
        input.raw_name,
        input.substring_start,
        input.substring_len,
        input.replace_from,
        input.replace_to,
        input.version,
        input.minimum_version,
        input.start,
        input.operator,
        input.left_record,
        input.right_record,
    ) {
        NixEvalClosurePolicyOutcome::Accepted { .. } => accepted(IntegrationAdapterDomain::NixClosurePolicy),
        NixEvalClosurePolicyOutcome::Rejected { .. } => {
            rejected(IntegrationAdapterDomain::NixClosurePolicy, IntegrationAdapterRejection::NixPolicyRejected)
        }
    }
}

// r[impl integration-adapters.user-facing-adapters]
pub fn nix_closure_policy_adapter(input: NixClosurePolicyAdapterInput<'_>) -> IntegrationAdapterDecision {
    nix_closure_policy_adapter_core(input)
}

fn storage_wal_recovery_adapter_core(input: StorageWalRecoveryAdapterInput) -> StorageWalRecoveryAdapterDecision {
    let (replay_start, replay_end) = replay_range(input.checkpoint_lsn, input.wal_end);
    let replay_count = replay_count(input.checkpoint_lsn, input.wal_end);
    let recovery_needed = needs_recovery(input.checkpoint_lsn, input.wal_end);
    let decision = if can_append(input.wal_end, input.append_count) {
        accepted(IntegrationAdapterDomain::StorageWalRecovery)
    } else {
        rejected(IntegrationAdapterDomain::StorageWalRecovery, IntegrationAdapterRejection::WalAppendWouldOverflow)
    };
    StorageWalRecoveryAdapterDecision {
        decision,
        replay_start,
        replay_end,
        replay_count,
        needs_recovery: recovery_needed,
    }
}

// r[impl integration-adapters.user-facing-adapters]
pub fn storage_wal_recovery_adapter(input: StorageWalRecoveryAdapterInput) -> StorageWalRecoveryAdapterDecision {
    storage_wal_recovery_adapter_core(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nickel_priority::MergePriority;
    use crate::ucan_token::UcanCapability;

    fn valid_payload() -> UcanPayload {
        UcanPayload {
            iss: b"did:key:issuer".to_vec(),
            aud: b"did:key:audience".to_vec(),
            att: vec![UcanCapability {
                resource: b"storage://bucket".to_vec(),
                ability: b"crud/read".to_vec(),
            }],
            prf: Vec::new(),
            exp: 100,
            nbf: 10,
        }
    }

    fn closure_item(key: u64) -> ClosureItem {
        ClosureItem { key, data: key * 10 }
    }

    fn exec_field(key: &[u8], value: u64) -> ExecField {
        ExecField {
            key: key.to_vec(),
            value,
            priority: MergePriority::Neutral,
        }
    }

    struct NixFixture {
        base_attrs: Vec<(Vec<u8>, u64)>,
        overlay_attrs: Vec<(Vec<u8>, u64)>,
        required: Vec<u64>,
        batches: Vec<Vec<u64>>,
        raw_name: Vec<u8>,
        replace_from: Vec<Vec<u8>>,
        replace_to: Vec<Vec<u8>>,
        version: Vec<u8>,
        minimum_version: Vec<u8>,
        start: Vec<ClosureItem>,
        operator: Vec<(u64, Vec<ClosureItem>)>,
        left_record: Vec<ExecField>,
        right_record: Vec<ExecField>,
    }

    impl NixFixture {
        fn valid() -> Self {
            Self {
                base_attrs: vec![(b"dep-a".to_vec(), 1)],
                overlay_attrs: vec![(b"dep-b".to_vec(), 2)],
                required: vec![1, 2, 3],
                batches: vec![vec![1, 2], vec![3]],
                raw_name: b"pkg-name".to_vec(),
                replace_from: vec![b"pkg".to_vec()],
                replace_to: vec![b"drv".to_vec()],
                version: b"2.0".to_vec(),
                minimum_version: b"1.0".to_vec(),
                start: vec![closure_item(1)],
                operator: vec![(1, vec![closure_item(2)]), (2, vec![closure_item(3)])],
                left_record: vec![exec_field(b"a", 10)],
                right_record: vec![exec_field(b"b", 20)],
            }
        }

        fn input(&self) -> NixClosurePolicyAdapterInput<'_> {
            NixClosurePolicyAdapterInput {
                base_attrs: &self.base_attrs,
                overlay_attrs: &self.overlay_attrs,
                required_closure_keys: &self.required,
                dependency_batches: &self.batches,
                raw_name: &self.raw_name,
                substring_start: 0,
                substring_len: self.raw_name.len(),
                replace_from: &self.replace_from,
                replace_to: &self.replace_to,
                version: &self.version,
                minimum_version: &self.minimum_version,
                start: &self.start,
                operator: &self.operator,
                left_record: &self.left_record,
                right_record: &self.right_record,
            }
        }
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn consensus_read_accepts_applied_index() {
        let decision = consensus_read_adapter(ConsensusReadAdapterInput {
            read_log_index: 7,
            last_applied_index: 7,
        });
        assert_eq!(decision, accepted(IntegrationAdapterDomain::ConsensusRead));
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn consensus_read_rejects_apply_lag() {
        let decision = consensus_read_adapter(ConsensusReadAdapterInput {
            read_log_index: 8,
            last_applied_index: 7,
        });
        assert_eq!(
            decision,
            rejected(IntegrationAdapterDomain::ConsensusRead, IntegrationAdapterRejection::ApplyCatchupRequired,)
        );
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn ucan_authorization_accepts_valid_envelope() {
        let payload = valid_payload();
        let decision = ucan_authorization_adapter(UcanAuthorizationAdapterInput {
            payload: &payload,
            signature_valid: true,
            now: 50,
            tolerance: 0,
        });
        assert_eq!(decision, accepted(IntegrationAdapterDomain::UcanAuthorization));
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn ucan_authorization_rejects_invalid_signature() {
        let payload = valid_payload();
        let decision = ucan_authorization_adapter(UcanAuthorizationAdapterInput {
            payload: &payload,
            signature_valid: false,
            now: 50,
            tolerance: 0,
        });
        assert_eq!(
            decision,
            rejected(IntegrationAdapterDomain::UcanAuthorization, IntegrationAdapterRejection::InvalidUcanEnvelope,)
        );
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn nix_policy_accepts_valid_closure() {
        let fixture = NixFixture::valid();
        let decision = nix_closure_policy_adapter(fixture.input());
        assert_eq!(decision, accepted(IntegrationAdapterDomain::NixClosurePolicy));
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn nix_policy_rejects_duplicate_overlay_key() {
        let mut fixture = NixFixture::valid();
        fixture.overlay_attrs = vec![(b"dep-b".to_vec(), 2), (b"dep-b".to_vec(), 3)];
        let decision = nix_closure_policy_adapter(fixture.input());
        assert_eq!(
            decision,
            rejected(IntegrationAdapterDomain::NixClosurePolicy, IntegrationAdapterRejection::NixPolicyRejected,)
        );
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn storage_wal_recovery_accepts_safe_append() {
        let decision = storage_wal_recovery_adapter(StorageWalRecoveryAdapterInput {
            checkpoint_lsn: 3,
            wal_end: 8,
            append_count: 2,
        });
        assert_eq!(decision.decision, accepted(IntegrationAdapterDomain::StorageWalRecovery));
        assert_eq!(decision.replay_start, 3);
        assert_eq!(decision.replay_end, 8);
        assert_eq!(decision.replay_count, 5);
        assert!(decision.needs_recovery);
    }

    // r[verify integration-adapters.user-facing-adapters]
    #[test]
    fn storage_wal_recovery_rejects_overflow_append() {
        let decision = storage_wal_recovery_adapter(StorageWalRecoveryAdapterInput {
            checkpoint_lsn: u64::MAX,
            wal_end: u64::MAX,
            append_count: 1,
        });
        assert_eq!(
            decision.decision,
            rejected(IntegrationAdapterDomain::StorageWalRecovery, IntegrationAdapterRejection::WalAppendWouldOverflow,)
        );
        assert_eq!(decision.replay_count, 0);
        assert!(!decision.needs_recovery);
    }
}
