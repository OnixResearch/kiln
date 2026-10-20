//! Verified core Raft consensus primitives for the domain-primitives Raft OpenSpec.
//!
//! This module intentionally stays at the exact finite model boundary: term
//! update, vote/request-vote admission, and election-safety composition over
//! existing verified quorum/ballot/ghost-history primitives. It does not claim
//! networking, storage durability, wall-clock timing, byzantine tolerance, or
//! liveness.

#![allow(clippy::wildcard_imports)]

use vstd::prelude::*;

use crate::append_entries_check::*;
use crate::ballot::*;
use crate::election_safety_proof::*;
use crate::epoch::*;
use crate::ghost_history::*;
use crate::log::*;
use crate::quorum::*;

verus! {

/// Minimal term-bearing local state used by the finite Raft model slice.
pub struct RaftTermState {
    /// The node's current term.
    pub current_term: u64,
}

/// Explicit finite Raft log-entry envelope for the domain primitive model.
#[derive(Copy, Clone, Debug)]
pub struct RaftIndexedLogEntry {
    /// One-based Raft log index.
    pub index: u64,
    /// Election term that produced the entry.
    pub term: u64,
    /// Finite command identifier; this model does not claim client/application semantics.
    pub command: u64,
}

/// Finite persistent-state envelope; durability/fsync is intentionally out of scope.
#[derive(Copy, Clone, Debug)]
pub struct RaftPersistentMetadata {
    /// Persisted current term.
    pub current_term: u64,
    /// Persisted voted-for node; zero means no vote recorded in this finite model.
    pub voted_for: u64,
}

/// Finite volatile-state envelope for commit/applied/log indexes.
#[derive(Copy, Clone, Debug)]
pub struct RaftVolatileMetadata {
    /// Highest committed one-based log index known to the node.
    pub commit_index: u64,
    /// Highest one-based log index applied to the local state-machine model.
    pub last_applied: u64,
    /// Highest one-based log index present in the local log metadata.
    pub last_log_index: u64,
}

/// Finite snapshot metadata; transfer, persistence, and application snapshot semantics are out of scope.
#[derive(Copy, Clone, Debug)]
pub struct RaftSnapshotMetadata {
    /// One-based last included log index; zero represents an empty snapshot boundary.
    pub last_included_index: usize,
    /// Term at the last included log index; zero when the boundary index is zero.
    pub last_included_term: u64,
    /// Finite configuration epoch attached to the snapshot metadata.
    pub config_epoch: u64,
    /// Finite state-machine summary identifier without claiming application semantics.
    pub state_machine_summary: u64,
}

/// Finite Raft membership/configuration metadata for exact quorum admission.
#[derive(Copy, Clone, Debug)]
pub struct RaftMembershipConfig {
    /// Monotone finite configuration epoch.
    pub epoch: u64,
    /// Number of voters in the old/current configuration.
    pub old_voter_count: u64,
    /// Number of voters in the new configuration; equals old voters for non-joint configs.
    pub new_voter_count: u64,
    /// Whether this configuration is in Raft joint-consensus mode.
    pub is_joint: bool,
    /// Upper bound for modeled one-based node identifiers / voter cardinalities.
    pub max_node_id: u64,
}


/// Finite OpenRaft-inspired log pointer metadata without storage durability claims.
#[derive(Copy, Clone, Debug)]
pub struct RaftOperationalLogPointers {
    /// Last purged log index, inclusive.
    pub purged: u64,
    /// Last snapshot-included log index, inclusive.
    pub snapshot: u64,
    /// Last state-machine-applied log index, inclusive.
    pub applied: u64,
    /// Last committed log index, inclusive.
    pub committed: u64,
    /// Last flushed/persisted log index metadata, inclusive.
    pub flushed: u64,
    /// Last submitted-to-storage log index metadata, inclusive.
    pub submitted: u64,
    /// Last accepted-from-leader log index metadata, inclusive.
    pub accepted: u64,
}

/// Finite three-stage I/O progress metadata.
#[derive(Copy, Clone, Debug)]
pub struct RaftIoProgressMetadata {
    /// Last flushed finite I/O sequence number.
    pub flushed: u64,
    /// Last submitted finite I/O sequence number.
    pub submitted: u64,
    /// Last accepted finite I/O sequence number.
    pub accepted: u64,
}

/// Finite vote/log I/O ordering metadata; no fsync or crash-recovery claim is made.
#[derive(Copy, Clone, Debug)]
pub struct RaftVoteLogIoMetadata {
    /// Highest term whose vote/term metadata has been persisted in this finite model.
    pub persisted_term: u64,
    /// Term of the log write being admitted.
    pub log_term: u64,
}

/// Finite committed/effective membership metadata inspired by OpenRaft.
#[derive(Copy, Clone, Debug)]
pub struct RaftEffectiveMembershipMetadata {
    /// Last committed configuration epoch.
    pub committed_epoch: u64,
    /// Currently effective configuration epoch, which may be uncommitted.
    pub effective_epoch: u64,
}

/// Finite replication-session metadata for isolating stale progress updates.
#[derive(Copy, Clone, Debug)]
pub struct RaftReplicationSessionMetadata {
    /// Leader term that owns the replication session.
    pub leader_term: u64,
    /// One-based leader node identifier.
    pub leader_node: u64,
    /// Log index of the membership entry governing this session; zero means initial config.
    pub membership_log_index: u64,
}

/// Finite read-linearizer metadata; no network quorum or clock lease claim is made.
#[derive(Copy, Clone, Debug)]
pub struct RaftReadLinearizerMetadata {
    /// Log index required before a modeled read may observe state.
    pub read_log_index: u64,
    /// Local last-applied log index.
    pub last_applied_index: u64,
}

/// Finite Raft message envelope kind constants.
pub const RAFT_MESSAGE_REQUEST_VOTE: u64 = 1;
pub const RAFT_MESSAGE_REQUEST_VOTE_RESPONSE: u64 = 2;
pub const RAFT_MESSAGE_APPEND_ENTRIES: u64 = 3;
pub const RAFT_MESSAGE_APPEND_ENTRIES_RESPONSE: u64 = 4;
pub const RAFT_MESSAGE_INSTALL_SNAPSHOT: u64 = 5;
pub const RAFT_MESSAGE_LOCAL_TIMEOUT: u64 = 6;
pub const RAFT_MESSAGE_CLIENT_PROPOSAL: u64 = 7;

/// Deterministic Raft admission success/failure codes.
pub const RAFT_ADMISSION_OK: u64 = 0;
pub const RAFT_ADMISSION_BEFORE_INVARIANT_FAILED: u64 = 1;
pub const RAFT_ADMISSION_MESSAGE_FAILED: u64 = 2;
pub const RAFT_ADMISSION_TRACE_FAILED: u64 = 3;
pub const RAFT_ADMISSION_AFTER_INVARIANT_FAILED: u64 = 4;

/// Finite term-bearing Raft message/input envelope; transport delivery/order is out of scope.
#[derive(Copy, Clone, Debug)]
pub struct RaftMessageEnvelope {
    /// One of the RAFT_MESSAGE_* finite kind constants.
    pub kind: u64,
    /// Term attached to this modeled input.
    pub term: u64,
    /// One-based source node, or client/local actor id in this finite model.
    pub source: u64,
    /// One-based destination node.
    pub target: u64,
    /// Finite log/snapshot index metadata used by message-specific predicates.
    pub index: u64,
    /// Finite command or response identifier; no application semantics are claimed.
    pub payload: u64,
}

/// Finite adjacent Raft trace step envelope.
#[derive(Copy, Clone, Debug)]
pub struct RaftTransitionTraceStep {
    /// Receiver term before evaluating the envelope.
    pub before_term: u64,
    /// Receiver term after evaluating the envelope.
    pub after_term: u64,
    /// Finite input envelope for this trace step.
    pub message: RaftMessageEnvelope,
    /// Whether this modeled transition was admitted rather than rejected.
    pub admitted: bool,
}

/// Copyable metadata bundle for finite Raft invariant composition.
#[derive(Copy, Clone, Debug)]
pub struct RaftInvariantBundleMetadata {
    /// Local current term used by transition admission.
    pub current_term: u64,
    /// Finite persisted term/vote metadata.
    pub persistent: RaftPersistentMetadata,
    /// Finite volatile applied/commit/log-index metadata.
    pub volatile: RaftVolatileMetadata,
    /// Finite snapshot boundary metadata.
    pub snapshot: RaftSnapshotMetadata,
    /// Finite membership/quorum configuration metadata.
    pub membership: RaftMembershipConfig,
    /// Modeled one-based node-universe bound for local metadata.
    pub max_node_id: u64,
}

/// Deterministic Raft admission result with stable first-failing-check metadata.
#[derive(Copy, Clone, Debug)]
pub struct RaftAdmissionResult {
    /// Whether the finite transition candidate is admitted.
    pub admitted: bool,
    /// Stable RAFT_ADMISSION_* code for success or the first rejected finite check.
    pub failing_check: u64,
}

/// Finite AppendEntries conflict metadata; zero index/term means no concrete conflicting entry.
#[derive(Copy, Clone, Debug)]
pub struct RaftAppendConflictMetadata {
    /// One-based index of the conflicting follower entry, or zero when absent.
    pub conflict_index: u64,
    /// Term at the conflict index, or zero when no concrete conflicting entry is identified.
    pub conflict_term: u64,
    /// One-based follower log length metadata, used to bound conflict indexes.
    pub follower_log_len: u64,
}

/// Finite follower log-update metadata; it records only modeled lengths and rejection state.
#[derive(Copy, Clone, Debug)]
pub struct RaftFollowerLogUpdateMetadata {
    /// One-based previous-log prefix length retained from the follower log on accepted updates.
    pub prev_log_index: u64,
    /// Number of admitted entries appended after the retained prefix.
    pub appended_len: u64,
    /// Resulting follower-log length metadata after evaluating the update.
    pub successor_len: u64,
    /// Whether AppendEntries admission rejected the update before log mutation.
    pub rejected: bool,
}

/// Stable rejection code: all finite model envelope checks passed.
pub const RAFT_MODEL_OK: u64 = 0;
/// Stable rejection code: node id is zero or above the modeled universe.
pub const RAFT_MODEL_UNKNOWN_NODE: u64 = 1;
/// Stable rejection code: term exceeds the finite model bound.
pub const RAFT_MODEL_TERM_OUT_OF_BOUNDS: u64 = 2;
/// Stable rejection code: log index exceeds the finite model bound.
pub const RAFT_MODEL_INDEX_OUT_OF_BOUNDS: u64 = 3;
/// Stable rejection code: command id exceeds the finite model bound.
pub const RAFT_MODEL_COMMAND_OUT_OF_BOUNDS: u64 = 4;
/// Stable rejection code: message kind is outside the modeled Raft input classes.
pub const RAFT_MODEL_MESSAGE_KIND_OUT_OF_BOUNDS: u64 = 5;
/// Stable rejection code: configuration epoch exceeds the finite model bound.
pub const RAFT_MODEL_CONFIG_EPOCH_OUT_OF_BOUNDS: u64 = 6;

/// Stable finite node role constant for follower state.
pub const RAFT_NODE_ROLE_FOLLOWER: u64 = 1;
/// Stable finite node role constant for candidate state.
pub const RAFT_NODE_ROLE_CANDIDATE: u64 = 2;
/// Stable finite node role constant for leader state.
pub const RAFT_NODE_ROLE_LEADER: u64 = 3;

/// Stable node-state rejection code: all finite node-state checks passed.
pub const RAFT_NODE_STATE_OK: u64 = 0;
/// Stable node-state rejection code: base model admission failed.
pub const RAFT_NODE_STATE_MODEL_FAILED: u64 = 1;
/// Stable node-state rejection code: role is outside follower/candidate/leader.
pub const RAFT_NODE_STATE_INVALID_ROLE: u64 = 2;
/// Stable node-state rejection code: leader role term differs from current term.
pub const RAFT_NODE_STATE_INCONSISTENT_LEADER_TERM: u64 = 3;
/// Stable node-state rejection code: last-applied is after commit.
pub const RAFT_NODE_STATE_APPLIED_AFTER_COMMIT: u64 = 4;
/// Stable node-state rejection code: commit is after the local log.
pub const RAFT_NODE_STATE_COMMIT_AFTER_LOG: u64 = 5;
/// Stable node-state rejection code: candidate did not vote for itself in its candidate term.
pub const RAFT_NODE_STATE_INCONSISTENT_CANDIDATE_VOTE: u64 = 6;

/// Stable applied-prefix rejection code: all finite apply checks passed.
pub const RAFT_APPLIED_PREFIX_OK: u64 = 0;
/// Stable applied-prefix rejection code: application would move backwards.
pub const RAFT_APPLIED_PREFIX_REGRESSION: u64 = 1;
/// Stable applied-prefix rejection code: target is after committed index.
pub const RAFT_APPLIED_PREFIX_UNCOMMITTED: u64 = 2;
/// Stable applied-prefix rejection code: target is after local log.
pub const RAFT_APPLIED_PREFIX_AFTER_LOG: u64 = 3;
/// Stable applied-prefix rejection code: step skips the contiguous prefix start.
pub const RAFT_APPLIED_PREFIX_SKIPPED: u64 = 4;

/// Stable client-proposal rejection code: proposal is append-eligible.
pub const RAFT_CLIENT_PROPOSAL_OK: u64 = 0;
/// Stable client-proposal rejection code: proposal/node is outside the finite model.
pub const RAFT_CLIENT_PROPOSAL_MODEL_FAILED: u64 = 1;
/// Stable client-proposal rejection code: target node-state was not admitted.
pub const RAFT_CLIENT_PROPOSAL_NODE_STATE_FAILED: u64 = 2;
/// Stable client-proposal rejection code: proposal target is not the supplied node state.
pub const RAFT_CLIENT_PROPOSAL_WRONG_TARGET: u64 = 3;
/// Stable client-proposal rejection code: target is not an admitted leader.
pub const RAFT_CLIENT_PROPOSAL_NOT_LEADER: u64 = 4;
/// Stable client-proposal rejection code: proposal term is stale for the leader.
pub const RAFT_CLIENT_PROPOSAL_STALE_TERM: u64 = 5;
/// Stable client-proposal rejection code: proposal term is newer than the leader state.
pub const RAFT_CLIENT_PROPOSAL_FUTURE_TERM: u64 = 6;

/// Stable OpenRaft-inspired admission code: all finite operational checks passed.
pub const RAFT_OPENRAFT_OK: u64 = 0;
/// Stable OpenRaft-inspired admission code: purged pointer is after snapshot pointer.
pub const RAFT_OPENRAFT_PURGED_AFTER_SNAPSHOT: u64 = 1;
/// Stable OpenRaft-inspired admission code: snapshot pointer is after applied pointer.
pub const RAFT_OPENRAFT_SNAPSHOT_AFTER_APPLIED: u64 = 2;
/// Stable OpenRaft-inspired admission code: applied pointer is after committed pointer.
pub const RAFT_OPENRAFT_APPLIED_AFTER_COMMITTED: u64 = 3;
/// Stable OpenRaft-inspired admission code: committed pointer is after submitted pointer.
pub const RAFT_OPENRAFT_COMMITTED_AFTER_SUBMITTED: u64 = 4;
/// Stable OpenRaft-inspired admission code: flushed pointer is after submitted pointer.
pub const RAFT_OPENRAFT_FLUSHED_AFTER_SUBMITTED: u64 = 5;
/// Stable OpenRaft-inspired admission code: submitted pointer is after accepted pointer.
pub const RAFT_OPENRAFT_SUBMITTED_AFTER_ACCEPTED: u64 = 6;
/// Stable OpenRaft-inspired admission code: existing progress metadata is invalid.
pub const RAFT_OPENRAFT_OLD_PROGRESS_INVALID: u64 = 7;
/// Stable OpenRaft-inspired admission code: new progress metadata is invalid.
pub const RAFT_OPENRAFT_NEW_PROGRESS_INVALID: u64 = 8;
/// Stable OpenRaft-inspired admission code: flushed cursor regressed.
pub const RAFT_OPENRAFT_FLUSHED_REGRESSED: u64 = 9;
/// Stable OpenRaft-inspired admission code: submitted cursor regressed.
pub const RAFT_OPENRAFT_SUBMITTED_REGRESSED: u64 = 10;
/// Stable OpenRaft-inspired admission code: accepted cursor regressed.
pub const RAFT_OPENRAFT_ACCEPTED_REGRESSED: u64 = 11;
/// Stable OpenRaft-inspired admission code: vote/term metadata has not been ordered before log I/O.
pub const RAFT_OPENRAFT_PRIOR_TERM_METADATA_REQUIRED: u64 = 12;
/// Stable OpenRaft-inspired admission code: effective membership is older than committed membership.
pub const RAFT_OPENRAFT_EFFECTIVE_BEFORE_COMMITTED: u64 = 13;
/// Stable OpenRaft-inspired admission code: replication update belongs to a stale session.
pub const RAFT_OPENRAFT_STALE_REPLICATION_SESSION: u64 = 14;
/// Stable OpenRaft-inspired admission code: replication matched index moved backwards.
pub const RAFT_OPENRAFT_MATCHED_INDEX_REGRESSED: u64 = 15;
/// Stable OpenRaft-inspired admission code: replication matched index is beyond leader log.
pub const RAFT_OPENRAFT_MATCHED_INDEX_AFTER_LEADER_LOG: u64 = 16;
/// Stable OpenRaft-inspired admission code: local apply has not caught up to the read index.
pub const RAFT_OPENRAFT_APPLY_CATCHUP_REQUIRED: u64 = 17;

/// Finite Raft model envelope. Bounds are caller-provided model limits, not runtime resource claims.
#[derive(Copy, Clone, Debug)]
pub struct RaftModelEnvelope {
    /// Highest modeled one-based node id.
    pub max_node_id: u64,
    /// Highest modeled term.
    pub max_term: u64,
    /// Highest modeled one-based log index.
    pub max_log_index: u64,
    /// Highest modeled command identifier.
    pub max_command_id: u64,
    /// Highest modeled configuration epoch.
    pub max_config_epoch: u64,
}

/// Deterministic finite model admission result.
#[derive(Copy, Clone, Debug)]
pub struct RaftModelAdmissionResult {
    /// Whether all finite model checks passed.
    pub admitted: bool,
    /// Stable RAFT_MODEL_* code for success or the first rejected field.
    pub failing_check: u64,
}

/// Finite node-state envelope for follower/candidate/leader metadata.
#[derive(Copy, Clone, Debug)]
pub struct RaftNodeStateMetadata {
    /// One of the RAFT_NODE_ROLE_* constants.
    pub role: u64,
    /// One-based node id.
    pub node_id: u64,
    /// Current local term.
    pub current_term: u64,
    /// Term attached to the current role witness.
    pub role_term: u64,
    /// Recorded vote in the current term; zero means no vote.
    pub voted_for: u64,
    /// Last local log term metadata.
    pub last_log_term: u64,
    /// Last local log index metadata.
    pub last_log_index: u64,
    /// Highest committed local log index.
    pub commit_index: u64,
    /// Highest applied local log index.
    pub last_applied: u64,
    /// Configuration epoch attached to the local state metadata.
    pub config_epoch: u64,
}

/// Deterministic finite node-state admission result.
#[derive(Copy, Clone, Debug)]
pub struct RaftNodeStateAdmissionResult {
    /// Whether all finite node-state checks passed.
    pub admitted: bool,
    /// Stable RAFT_NODE_STATE_* code for success or the first rejected field.
    pub failing_check: u64,
}

/// Finite applied-prefix transition metadata. It models contiguous apply progress only.
#[derive(Copy, Clone, Debug)]
pub struct RaftAppliedPrefixMetadata {
    /// Previous last-applied index.
    pub old_last_applied: u64,
    /// First index applied by this step, or zero for a no-op step.
    pub first_applied: u64,
    /// New last-applied index after the step.
    pub new_last_applied: u64,
    /// Highest committed index admitted by Raft metadata.
    pub commit_index: u64,
    /// Highest local log index.
    pub last_log_index: u64,
}

/// Deterministic finite applied-prefix admission result.
#[derive(Copy, Clone, Debug)]
pub struct RaftAppliedPrefixAdmissionResult {
    /// Whether the apply step is contiguous, committed, and local-log-bounded.
    pub admitted: bool,
    /// Stable RAFT_APPLIED_PREFIX_* code for success or the first rejected field.
    pub failing_check: u64,
}

/// Finite client proposal envelope; this does not model client retries or application execution.
#[derive(Copy, Clone, Debug)]
pub struct RaftClientProposalEnvelope {
    /// One-based target Raft node.
    pub target_node: u64,
    /// Proposal term as observed by the client-facing model.
    pub proposal_term: u64,
    /// Finite command identifier.
    pub command_id: u64,
}

/// Deterministic finite client-proposal admission result.
#[derive(Copy, Clone, Debug)]
pub struct RaftClientProposalAdmissionResult {
    /// Whether the proposal is eligible for the modeled leader append path.
    pub admitted: bool,
    /// Stable RAFT_CLIENT_PROPOSAL_* code for success or the first rejected field.
    pub failing_check: u64,
}

/// Deterministic finite OpenRaft-inspired operational admission result.
#[derive(Copy, Clone, Debug)]
pub struct RaftOpenraftAdmissionResult {
    /// Whether all finite OpenRaft-inspired operational checks passed.
    pub admitted: bool,
    /// Stable RAFT_OPENRAFT_* code for success or the first rejected field.
    pub failing_check: u64,
}

// r[depends domain-primitives.raft-consensus.model]
/// Spec: stable finite model rejection codes are drawn from the modeled code set.
pub open spec fn raft_model_code_valid_spec(code: u64) -> bool {
    code <= RAFT_MODEL_CONFIG_EPOCH_OUT_OF_BOUNDS
}

// r[depends domain-primitives.raft-consensus.model]
/// Spec: the finite model envelope itself has non-empty node and term universes.
pub open spec fn raft_model_envelope_valid_spec(model: RaftModelEnvelope) -> bool {
    model.max_node_id > 0 && model.max_term > 0
}

// r[depends domain-primitives.raft-consensus.model]
/// Spec: a one-based node id is inside the finite Raft model envelope.
pub open spec fn raft_model_node_valid_spec(model: RaftModelEnvelope, node_id: u64) -> bool {
    raft_model_envelope_valid_spec(model) && 1 <= node_id && node_id <= model.max_node_id
}

// r[depends domain-primitives.raft-consensus.model]
/// Spec: referenced term/index/command/message/config metadata share one finite model boundary.
pub open spec fn raft_model_references_valid_spec(
    model: RaftModelEnvelope,
    node_id: u64,
    term: u64,
    log_index: u64,
    command_id: u64,
    message_kind: u64,
    config_epoch: u64,
) -> bool {
    raft_model_node_valid_spec(model, node_id)
        && term <= model.max_term
        && log_index <= model.max_log_index
        && command_id <= model.max_command_id
        && raft_message_kind_valid_spec(message_kind)
        && config_epoch <= model.max_config_epoch
}

// r[depends domain-primitives.raft-consensus.model]
/// Spec: deterministic model admission reports the first finite envelope failure.
pub open spec fn raft_model_admission_result_spec(
    model: RaftModelEnvelope,
    node_id: u64,
    term: u64,
    log_index: u64,
    command_id: u64,
    message_kind: u64,
    config_epoch: u64,
) -> RaftModelAdmissionResult {
    if !raft_model_node_valid_spec(model, node_id) {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_UNKNOWN_NODE }
    } else if term > model.max_term {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_TERM_OUT_OF_BOUNDS }
    } else if log_index > model.max_log_index {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_INDEX_OUT_OF_BOUNDS }
    } else if command_id > model.max_command_id {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_COMMAND_OUT_OF_BOUNDS }
    } else if !raft_message_kind_valid_spec(message_kind) {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_MESSAGE_KIND_OUT_OF_BOUNDS }
    } else if config_epoch > model.max_config_epoch {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_CONFIG_EPOCH_OUT_OF_BOUNDS }
    } else {
        RaftModelAdmissionResult { admitted: true, failing_check: RAFT_MODEL_OK }
    }
}

// r[depends domain-primitives.raft-consensus.node-state]
/// Spec: finite node role is follower, candidate, or leader.
pub open spec fn raft_node_role_valid_spec(role: u64) -> bool {
    RAFT_NODE_ROLE_FOLLOWER <= role && role <= RAFT_NODE_ROLE_LEADER
}

// r[depends domain-primitives.raft-consensus.node-state]
/// Spec: deterministic node-state admission reports the first finite consistency failure.
pub open spec fn raft_node_state_admission_result_spec(
    model: RaftModelEnvelope,
    state: RaftNodeStateMetadata,
) -> RaftNodeStateAdmissionResult {
    if !raft_model_references_valid_spec(
        model,
        state.node_id,
        state.current_term,
        state.last_log_index,
        state.voted_for,
        RAFT_MESSAGE_APPEND_ENTRIES,
        state.config_epoch,
    ) || state.voted_for > model.max_node_id || state.role_term > model.max_term || state.last_log_term > model.max_term {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_MODEL_FAILED }
    } else if !raft_node_role_valid_spec(state.role) {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_INVALID_ROLE }
    } else if state.role == RAFT_NODE_ROLE_LEADER && state.role_term != state.current_term {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_INCONSISTENT_LEADER_TERM }
    } else if state.last_applied > state.commit_index {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_APPLIED_AFTER_COMMIT }
    } else if state.commit_index > state.last_log_index {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_COMMIT_AFTER_LOG }
    } else if state.role == RAFT_NODE_ROLE_CANDIDATE && (state.role_term != state.current_term || state.voted_for != state.node_id) {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_INCONSISTENT_CANDIDATE_VOTE }
    } else {
        RaftNodeStateAdmissionResult { admitted: true, failing_check: RAFT_NODE_STATE_OK }
    }
}

// r[depends domain-primitives.raft-consensus.node-state]
/// Spec: finite node-state admission is true exactly for the OK result.
pub open spec fn raft_node_state_admit_spec(model: RaftModelEnvelope, state: RaftNodeStateMetadata) -> bool {
    raft_node_state_admission_result_spec(model, state).admitted
}

// r[depends domain-primitives.raft-consensus.applied-prefix]
/// Spec: deterministic applied-prefix admission reports the first prefix/commit/log failure.
pub open spec fn raft_applied_prefix_admission_result_spec(
    step: RaftAppliedPrefixMetadata,
) -> RaftAppliedPrefixAdmissionResult {
    if step.new_last_applied < step.old_last_applied {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_REGRESSION }
    } else if step.new_last_applied > step.commit_index {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_UNCOMMITTED }
    } else if step.new_last_applied > step.last_log_index {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_AFTER_LOG }
    } else if step.new_last_applied == step.old_last_applied {
        if step.first_applied == 0 {
            RaftAppliedPrefixAdmissionResult { admitted: true, failing_check: RAFT_APPLIED_PREFIX_OK }
        } else {
            RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_SKIPPED }
        }
    } else if step.first_applied == step.old_last_applied + 1 {
        RaftAppliedPrefixAdmissionResult { admitted: true, failing_check: RAFT_APPLIED_PREFIX_OK }
    } else {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_SKIPPED }
    }
}

// r[depends domain-primitives.raft-consensus.applied-prefix]
/// Spec: finite applied-prefix admission is true exactly for the OK result.
pub open spec fn raft_applied_prefix_admit_spec(step: RaftAppliedPrefixMetadata) -> bool {
    raft_applied_prefix_admission_result_spec(step).admitted
}

// r[depends domain-primitives.raft-consensus.client-proposal]
/// Spec: deterministic client-proposal admission reports the first leader/term/model failure.
pub open spec fn raft_client_proposal_admission_result_spec(
    model: RaftModelEnvelope,
    proposal: RaftClientProposalEnvelope,
    target_state: RaftNodeStateMetadata,
) -> RaftClientProposalAdmissionResult {
    if !raft_model_references_valid_spec(
        model,
        proposal.target_node,
        proposal.proposal_term,
        target_state.last_log_index,
        proposal.command_id,
        RAFT_MESSAGE_CLIENT_PROPOSAL,
        target_state.config_epoch,
    ) {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_MODEL_FAILED }
    } else if !raft_node_state_admit_spec(model, target_state) {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_NODE_STATE_FAILED }
    } else if proposal.target_node != target_state.node_id {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_WRONG_TARGET }
    } else if target_state.role != RAFT_NODE_ROLE_LEADER {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_NOT_LEADER }
    } else if proposal.proposal_term < target_state.current_term {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_STALE_TERM }
    } else if proposal.proposal_term > target_state.current_term {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_FUTURE_TERM }
    } else {
        RaftClientProposalAdmissionResult { admitted: true, failing_check: RAFT_CLIENT_PROPOSAL_OK }
    }
}

// r[depends domain-primitives.raft-consensus.client-proposal]
/// Spec: finite client-proposal admission is true exactly for the OK result.
pub open spec fn raft_client_proposal_admit_spec(
    model: RaftModelEnvelope,
    proposal: RaftClientProposalEnvelope,
    target_state: RaftNodeStateMetadata,
) -> bool {
    raft_client_proposal_admission_result_spec(model, proposal, target_state).admitted
}

// r[impl domain-primitives.raft-consensus.model]
/// Return whether a finite model rejection code is valid.
pub fn raft_model_code_valid(code: u64) -> (result: bool)
    ensures result == raft_model_code_valid_spec(code)
{
    code <= RAFT_MODEL_CONFIG_EPOCH_OUT_OF_BOUNDS
}

// r[impl domain-primitives.raft-consensus.model]
/// Return whether the finite model envelope has non-empty node and term universes.
pub fn raft_model_envelope_valid(model: &RaftModelEnvelope) -> (result: bool)
    ensures result == raft_model_envelope_valid_spec(*model)
{
    model.max_node_id > 0 && model.max_term > 0
}

// r[impl domain-primitives.raft-consensus.model]
/// Return deterministic model admission metadata for a finite Raft reference set.
pub fn raft_model_admission_result(
    model: &RaftModelEnvelope,
    node_id: u64,
    term: u64,
    log_index: u64,
    command_id: u64,
    message_kind: u64,
    config_epoch: u64,
) -> (result: RaftModelAdmissionResult)
    ensures
        result == raft_model_admission_result_spec(*model, node_id, term, log_index, command_id, message_kind, config_epoch),
        raft_model_code_valid_spec(result.failing_check),
        result.admitted ==> result.failing_check == RAFT_MODEL_OK,
        !result.admitted ==> result.failing_check != RAFT_MODEL_OK,
{
    if !(raft_model_envelope_valid(model) && 1 <= node_id && node_id <= model.max_node_id) {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_UNKNOWN_NODE }
    } else if term > model.max_term {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_TERM_OUT_OF_BOUNDS }
    } else if log_index > model.max_log_index {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_INDEX_OUT_OF_BOUNDS }
    } else if command_id > model.max_command_id {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_COMMAND_OUT_OF_BOUNDS }
    } else if !raft_message_kind_valid(message_kind) {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_MESSAGE_KIND_OUT_OF_BOUNDS }
    } else if config_epoch > model.max_config_epoch {
        RaftModelAdmissionResult { admitted: false, failing_check: RAFT_MODEL_CONFIG_EPOCH_OUT_OF_BOUNDS }
    } else {
        RaftModelAdmissionResult { admitted: true, failing_check: RAFT_MODEL_OK }
    }
}

// r[impl domain-primitives.raft-consensus.model]
/// Return whether a finite Raft reference set is admitted by the model envelope.
pub fn raft_model_references_valid(
    model: &RaftModelEnvelope,
    node_id: u64,
    term: u64,
    log_index: u64,
    command_id: u64,
    message_kind: u64,
    config_epoch: u64,
) -> (result: bool)
    ensures result == raft_model_references_valid_spec(*model, node_id, term, log_index, command_id, message_kind, config_epoch)
{
    raft_model_admission_result(model, node_id, term, log_index, command_id, message_kind, config_epoch).admitted
}

// r[impl domain-primitives.raft-consensus.node-state]
/// Return whether a finite node role is follower, candidate, or leader.
pub fn raft_node_role_valid(role: u64) -> (result: bool)
    ensures result == raft_node_role_valid_spec(role)
{
    RAFT_NODE_ROLE_FOLLOWER <= role && role <= RAFT_NODE_ROLE_LEADER
}

// r[impl domain-primitives.raft-consensus.node-state]
/// Return deterministic node-state admission metadata.
pub fn raft_node_state_admission_result(
    model: &RaftModelEnvelope,
    state: &RaftNodeStateMetadata,
) -> (result: RaftNodeStateAdmissionResult)
    ensures result == raft_node_state_admission_result_spec(*model, *state)
{
    if !raft_model_references_valid(model, state.node_id, state.current_term, state.last_log_index, state.voted_for, RAFT_MESSAGE_APPEND_ENTRIES, state.config_epoch)
        || state.voted_for > model.max_node_id
        || state.role_term > model.max_term
        || state.last_log_term > model.max_term
    {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_MODEL_FAILED }
    } else if !raft_node_role_valid(state.role) {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_INVALID_ROLE }
    } else if state.role == RAFT_NODE_ROLE_LEADER && state.role_term != state.current_term {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_INCONSISTENT_LEADER_TERM }
    } else if state.last_applied > state.commit_index {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_APPLIED_AFTER_COMMIT }
    } else if state.commit_index > state.last_log_index {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_COMMIT_AFTER_LOG }
    } else if state.role == RAFT_NODE_ROLE_CANDIDATE && (state.role_term != state.current_term || state.voted_for != state.node_id) {
        RaftNodeStateAdmissionResult { admitted: false, failing_check: RAFT_NODE_STATE_INCONSISTENT_CANDIDATE_VOTE }
    } else {
        RaftNodeStateAdmissionResult { admitted: true, failing_check: RAFT_NODE_STATE_OK }
    }
}

// r[impl domain-primitives.raft-consensus.node-state]
/// Return whether finite node-state metadata is admitted.
pub fn raft_node_state_admit(model: &RaftModelEnvelope, state: &RaftNodeStateMetadata) -> (result: bool)
    ensures result == raft_node_state_admit_spec(*model, *state)
{
    raft_node_state_admission_result(model, state).admitted
}

// r[impl domain-primitives.raft-consensus.applied-prefix]
/// Return deterministic applied-prefix admission metadata.
pub fn raft_applied_prefix_admission_result(
    step: &RaftAppliedPrefixMetadata,
) -> (result: RaftAppliedPrefixAdmissionResult)
    ensures result == raft_applied_prefix_admission_result_spec(*step)
{
    if step.new_last_applied < step.old_last_applied {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_REGRESSION }
    } else if step.new_last_applied > step.commit_index {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_UNCOMMITTED }
    } else if step.new_last_applied > step.last_log_index {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_AFTER_LOG }
    } else if step.new_last_applied == step.old_last_applied {
        if step.first_applied == 0 {
            RaftAppliedPrefixAdmissionResult { admitted: true, failing_check: RAFT_APPLIED_PREFIX_OK }
        } else {
            RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_SKIPPED }
        }
    } else if step.first_applied == step.old_last_applied + 1 {
        RaftAppliedPrefixAdmissionResult { admitted: true, failing_check: RAFT_APPLIED_PREFIX_OK }
    } else {
        RaftAppliedPrefixAdmissionResult { admitted: false, failing_check: RAFT_APPLIED_PREFIX_SKIPPED }
    }
}

// r[impl domain-primitives.raft-consensus.applied-prefix]
/// Return whether finite applied-prefix metadata is admitted.
pub fn raft_applied_prefix_admit(step: &RaftAppliedPrefixMetadata) -> (result: bool)
    ensures result == raft_applied_prefix_admit_spec(*step)
{
    raft_applied_prefix_admission_result(step).admitted
}

// r[impl domain-primitives.raft-consensus.client-proposal]
/// Return deterministic client-proposal admission metadata.
pub fn raft_client_proposal_admission_result(
    model: &RaftModelEnvelope,
    proposal: &RaftClientProposalEnvelope,
    target_state: &RaftNodeStateMetadata,
) -> (result: RaftClientProposalAdmissionResult)
    ensures result == raft_client_proposal_admission_result_spec(*model, *proposal, *target_state)
{
    if !raft_model_references_valid(model, proposal.target_node, proposal.proposal_term, target_state.last_log_index, proposal.command_id, RAFT_MESSAGE_CLIENT_PROPOSAL, target_state.config_epoch) {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_MODEL_FAILED }
    } else if !raft_node_state_admit(model, target_state) {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_NODE_STATE_FAILED }
    } else if proposal.target_node != target_state.node_id {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_WRONG_TARGET }
    } else if target_state.role != RAFT_NODE_ROLE_LEADER {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_NOT_LEADER }
    } else if proposal.proposal_term < target_state.current_term {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_STALE_TERM }
    } else if proposal.proposal_term > target_state.current_term {
        RaftClientProposalAdmissionResult { admitted: false, failing_check: RAFT_CLIENT_PROPOSAL_FUTURE_TERM }
    } else {
        RaftClientProposalAdmissionResult { admitted: true, failing_check: RAFT_CLIENT_PROPOSAL_OK }
    }
}

// r[impl domain-primitives.raft-consensus.client-proposal]
/// Return whether a finite client proposal is eligible for the modeled leader append path.
pub fn raft_client_proposal_admit(
    model: &RaftModelEnvelope,
    proposal: &RaftClientProposalEnvelope,
    target_state: &RaftNodeStateMetadata,
) -> (result: bool)
    ensures result == raft_client_proposal_admit_spec(*model, *proposal, *target_state)
{
    raft_client_proposal_admission_result(model, proposal, target_state).admitted
}

// r[verify domain-primitives.raft-consensus.model]
/// Admitted model references expose every finite bound they rely on.
pub proof fn admitted_model_references_are_bounded(
    model: RaftModelEnvelope,
    node_id: u64,
    term: u64,
    log_index: u64,
    command_id: u64,
    message_kind: u64,
    config_epoch: u64,
)
    requires raft_model_references_valid_spec(model, node_id, term, log_index, command_id, message_kind, config_epoch)
    ensures
        1 <= node_id <= model.max_node_id,
        term <= model.max_term,
        log_index <= model.max_log_index,
        command_id <= model.max_command_id,
        raft_message_kind_valid_spec(message_kind),
        config_epoch <= model.max_config_epoch,
{}

// r[verify domain-primitives.raft-consensus.model]
/// Unknown-node model failures are rejected without admitting the higher-level reference set.
pub proof fn model_unknown_node_result_rejects(
    model: RaftModelEnvelope,
    node_id: u64,
    term: u64,
    log_index: u64,
    command_id: u64,
    message_kind: u64,
    config_epoch: u64,
)
    requires !raft_model_node_valid_spec(model, node_id)
    ensures
        !raft_model_admission_result_spec(model, node_id, term, log_index, command_id, message_kind, config_epoch).admitted,
        raft_model_admission_result_spec(model, node_id, term, log_index, command_id, message_kind, config_epoch).failing_check == RAFT_MODEL_UNKNOWN_NODE,
{}

// r[verify domain-primitives.raft-consensus.node-state]
/// Admitted leader state belongs to the node's current term.
pub proof fn admitted_leader_state_has_current_term(model: RaftModelEnvelope, state: RaftNodeStateMetadata)
    requires
        raft_node_state_admit_spec(model, state),
        state.role == RAFT_NODE_ROLE_LEADER,
    ensures state.role_term == state.current_term
{}

// r[verify domain-primitives.raft-consensus.node-state]
/// Admitted node state never applies beyond commit or the local log.
pub proof fn admitted_node_state_orders_applied_commit_and_log(model: RaftModelEnvelope, state: RaftNodeStateMetadata)
    requires raft_node_state_admit_spec(model, state)
    ensures state.last_applied <= state.commit_index <= state.last_log_index
{}

// r[verify domain-primitives.raft-consensus.node-state]
/// Admitted candidate state records a self-vote in the candidate/current term.
pub proof fn admitted_candidate_state_records_self_vote(model: RaftModelEnvelope, state: RaftNodeStateMetadata)
    requires
        raft_node_state_admit_spec(model, state),
        state.role == RAFT_NODE_ROLE_CANDIDATE,
    ensures
        state.voted_for == state.node_id,
        state.role_term == state.current_term,
{}

// r[verify domain-primitives.raft-consensus.applied-prefix]
/// Admitted apply steps are monotone and bounded by commit and the local log.
pub proof fn admitted_applied_prefix_is_monotone_committed_and_local(step: RaftAppliedPrefixMetadata)
    requires raft_applied_prefix_admit_spec(step)
    ensures
        step.old_last_applied <= step.new_last_applied,
        step.new_last_applied <= step.commit_index,
        step.new_last_applied <= step.last_log_index,
{}

// r[verify domain-primitives.raft-consensus.applied-prefix]
/// Non-empty admitted apply steps start exactly at the next unapplied prefix index.
pub proof fn admitted_applied_prefix_starts_at_next_index(step: RaftAppliedPrefixMetadata)
    requires
        raft_applied_prefix_admit_spec(step),
        step.new_last_applied > step.old_last_applied,
    ensures step.first_applied == step.old_last_applied + 1
{}

// r[verify domain-primitives.raft-consensus.client-proposal]
/// Admitted client proposals target an admitted leader in the proposal/current term.
pub proof fn admitted_client_proposal_targets_current_leader(
    model: RaftModelEnvelope,
    proposal: RaftClientProposalEnvelope,
    target_state: RaftNodeStateMetadata,
)
    requires raft_client_proposal_admit_spec(model, proposal, target_state)
    ensures
        raft_node_state_admit_spec(model, target_state),
        proposal.target_node == target_state.node_id,
        target_state.role == RAFT_NODE_ROLE_LEADER,
        proposal.proposal_term == target_state.current_term,
{}

// r[verify domain-primitives.raft-consensus.client-proposal]
/// A proposal to a follower is rejected before append eligibility.
pub proof fn follower_client_proposal_rejects(
    model: RaftModelEnvelope,
    proposal: RaftClientProposalEnvelope,
    target_state: RaftNodeStateMetadata,
)
    requires
        raft_model_references_valid_spec(model, proposal.target_node, proposal.proposal_term, target_state.last_log_index, proposal.command_id, RAFT_MESSAGE_CLIENT_PROPOSAL, target_state.config_epoch),
        raft_node_state_admit_spec(model, target_state),
        proposal.target_node == target_state.node_id,
        target_state.role != RAFT_NODE_ROLE_LEADER,
    ensures
        !raft_client_proposal_admission_result_spec(model, proposal, target_state).admitted,
        raft_client_proposal_admission_result_spec(model, proposal, target_state).failing_check == RAFT_CLIENT_PROPOSAL_NOT_LEADER,
{}

// r[depends domain-primitives.raft-consensus.openraft-log-pointers]
// r[depends domain-primitives.raft-consensus.openraft-io-progress]
// r[depends domain-primitives.raft-consensus.openraft-vote-log-io]
// r[depends domain-primitives.raft-consensus.openraft-effective-membership]
// r[depends domain-primitives.raft-consensus.openraft-replication-session]
// r[depends domain-primitives.raft-consensus.openraft-read-linearizer]
/// Spec: stable OpenRaft-inspired operational admission codes are drawn from the modeled code set.
pub open spec fn raft_openraft_code_valid_spec(code: u64) -> bool {
    code <= RAFT_OPENRAFT_APPLY_CATCHUP_REQUIRED
}

// r[depends domain-primitives.raft-consensus.openraft-log-pointers]
/// Spec: OpenRaft-inspired log pointers are ordered in the finite safety lattice.
pub open spec fn raft_operational_log_pointers_valid_spec(ptrs: RaftOperationalLogPointers) -> bool {
    ptrs.purged <= ptrs.snapshot
        && ptrs.snapshot <= ptrs.applied
        && ptrs.applied <= ptrs.committed
        && ptrs.committed <= ptrs.submitted
        && ptrs.flushed <= ptrs.submitted
        && ptrs.submitted <= ptrs.accepted
}

// r[depends domain-primitives.raft-consensus.openraft-io-progress]
/// Spec: finite I/O progress cursors are monotone by stage.
pub open spec fn raft_io_progress_valid_spec(progress: RaftIoProgressMetadata) -> bool {
    progress.flushed <= progress.submitted && progress.submitted <= progress.accepted
}

// r[depends domain-primitives.raft-consensus.openraft-io-progress]
/// Spec: an I/O progress update is admitted only if each cursor stays monotone and valid.
pub open spec fn raft_io_progress_advance_admit_spec(
    old_progress: RaftIoProgressMetadata,
    new_progress: RaftIoProgressMetadata,
) -> bool {
    raft_io_progress_valid_spec(old_progress)
        && raft_io_progress_valid_spec(new_progress)
        && old_progress.flushed <= new_progress.flushed
        && old_progress.submitted <= new_progress.submitted
        && old_progress.accepted <= new_progress.accepted
}

// r[depends domain-primitives.raft-consensus.openraft-vote-log-io]
/// Spec: finite log I/O is admitted only after term/vote metadata for that log term is ordered first.
pub open spec fn raft_vote_log_io_ordered_spec(meta: RaftVoteLogIoMetadata) -> bool {
    meta.log_term <= meta.persisted_term
}

// r[depends domain-primitives.raft-consensus.openraft-effective-membership]
/// Spec: effective membership may be newer than the last committed membership but not older.
pub open spec fn raft_effective_membership_valid_spec(meta: RaftEffectiveMembershipMetadata) -> bool {
    meta.committed_epoch <= meta.effective_epoch
}

// r[depends domain-primitives.raft-consensus.openraft-effective-membership]
/// Spec: conflicting uncommitted effective membership rolls back to the committed epoch.
pub open spec fn raft_effective_membership_rollback_spec(
    meta: RaftEffectiveMembershipMetadata,
) -> RaftEffectiveMembershipMetadata {
    RaftEffectiveMembershipMetadata {
        committed_epoch: meta.committed_epoch,
        effective_epoch: meta.committed_epoch,
    }
}

// r[depends domain-primitives.raft-consensus.openraft-replication-session]
/// Spec: replication sessions match exactly on leader identity and membership log index.
pub open spec fn raft_replication_session_matches_spec(
    current: RaftReplicationSessionMetadata,
    update: RaftReplicationSessionMetadata,
) -> bool {
    current.leader_term == update.leader_term
        && current.leader_node == update.leader_node
        && current.membership_log_index == update.membership_log_index
}

// r[depends domain-primitives.raft-consensus.openraft-replication-session]
/// Spec: stale replication progress is admitted only for the current session and monotone match index.
pub open spec fn raft_replication_progress_update_admit_spec(
    current: RaftReplicationSessionMetadata,
    update_session: RaftReplicationSessionMetadata,
    old_matched_index: u64,
    new_matched_index: u64,
    leader_last_log_index: u64,
) -> bool {
    raft_replication_session_matches_spec(current, update_session)
        && old_matched_index <= new_matched_index
        && new_matched_index <= leader_last_log_index
}

// r[depends domain-primitives.raft-consensus.openraft-read-linearizer]
/// Spec: finite read-linearizer readiness holds once local apply has caught up to the read log index.
pub open spec fn raft_read_linearizer_ready_spec(meta: RaftReadLinearizerMetadata) -> bool {
    meta.read_log_index <= meta.last_applied_index
}

// r[depends domain-primitives.raft-consensus.openraft-log-pointers]
/// Spec: deterministic first-failure metadata for finite OpenRaft-inspired log-pointer admission.
pub open spec fn raft_openraft_log_pointers_admission_result_spec(
    ptrs: RaftOperationalLogPointers,
) -> RaftOpenraftAdmissionResult {
    if ptrs.purged > ptrs.snapshot {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_PURGED_AFTER_SNAPSHOT }
    } else if ptrs.snapshot > ptrs.applied {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_SNAPSHOT_AFTER_APPLIED }
    } else if ptrs.applied > ptrs.committed {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_APPLIED_AFTER_COMMITTED }
    } else if ptrs.committed > ptrs.submitted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_COMMITTED_AFTER_SUBMITTED }
    } else if ptrs.flushed > ptrs.submitted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_FLUSHED_AFTER_SUBMITTED }
    } else if ptrs.submitted > ptrs.accepted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_SUBMITTED_AFTER_ACCEPTED }
    } else {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    }
}

// r[depends domain-primitives.raft-consensus.openraft-io-progress]
/// Spec: deterministic first-failure metadata for finite OpenRaft-inspired I/O progress admission.
pub open spec fn raft_openraft_io_progress_admission_result_spec(
    old_progress: RaftIoProgressMetadata,
    new_progress: RaftIoProgressMetadata,
) -> RaftOpenraftAdmissionResult {
    if !raft_io_progress_valid_spec(old_progress) {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_OLD_PROGRESS_INVALID }
    } else if !raft_io_progress_valid_spec(new_progress) {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_NEW_PROGRESS_INVALID }
    } else if old_progress.flushed > new_progress.flushed {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_FLUSHED_REGRESSED }
    } else if old_progress.submitted > new_progress.submitted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_SUBMITTED_REGRESSED }
    } else if old_progress.accepted > new_progress.accepted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_ACCEPTED_REGRESSED }
    } else {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    }
}

// r[depends domain-primitives.raft-consensus.openraft-vote-log-io]
/// Spec: deterministic first-failure metadata for finite OpenRaft-inspired vote/log I/O ordering.
pub open spec fn raft_openraft_vote_log_io_admission_result_spec(
    meta: RaftVoteLogIoMetadata,
) -> RaftOpenraftAdmissionResult {
    if raft_vote_log_io_ordered_spec(meta) {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    } else {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_PRIOR_TERM_METADATA_REQUIRED }
    }
}

// r[depends domain-primitives.raft-consensus.openraft-effective-membership]
/// Spec: deterministic metadata for finite OpenRaft-inspired effective membership admission.
pub open spec fn raft_openraft_effective_membership_admission_result_spec(
    meta: RaftEffectiveMembershipMetadata,
) -> RaftOpenraftAdmissionResult {
    if raft_effective_membership_valid_spec(meta) {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    } else {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_EFFECTIVE_BEFORE_COMMITTED }
    }
}

// r[depends domain-primitives.raft-consensus.openraft-replication-session]
/// Spec: deterministic first-failure metadata for finite OpenRaft-inspired replication progress admission.
pub open spec fn raft_openraft_replication_progress_admission_result_spec(
    current: RaftReplicationSessionMetadata,
    update_session: RaftReplicationSessionMetadata,
    old_matched_index: u64,
    new_matched_index: u64,
    leader_last_log_index: u64,
) -> RaftOpenraftAdmissionResult {
    if !raft_replication_session_matches_spec(current, update_session) {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_STALE_REPLICATION_SESSION }
    } else if old_matched_index > new_matched_index {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_MATCHED_INDEX_REGRESSED }
    } else if new_matched_index > leader_last_log_index {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_MATCHED_INDEX_AFTER_LEADER_LOG }
    } else {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    }
}

// r[depends domain-primitives.raft-consensus.openraft-read-linearizer]
/// Spec: deterministic metadata for finite OpenRaft-inspired read-linearizer readiness.
pub open spec fn raft_openraft_read_linearizer_admission_result_spec(
    meta: RaftReadLinearizerMetadata,
) -> RaftOpenraftAdmissionResult {
    if raft_read_linearizer_ready_spec(meta) {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    } else {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_APPLY_CATCHUP_REQUIRED }
    }
}

// r[impl domain-primitives.raft-consensus.openraft-log-pointers]
/// Return whether OpenRaft-inspired finite log pointers satisfy their ordering lattice.
pub fn raft_operational_log_pointers_valid(ptrs: &RaftOperationalLogPointers) -> (result: bool)
    ensures result == raft_operational_log_pointers_valid_spec(*ptrs)
{
    ptrs.purged <= ptrs.snapshot
        && ptrs.snapshot <= ptrs.applied
        && ptrs.applied <= ptrs.committed
        && ptrs.committed <= ptrs.submitted
        && ptrs.flushed <= ptrs.submitted
        && ptrs.submitted <= ptrs.accepted
}

// r[impl domain-primitives.raft-consensus.openraft-io-progress]
/// Return whether finite I/O progress cursors are stage-ordered.
pub fn raft_io_progress_valid(progress: &RaftIoProgressMetadata) -> (result: bool)
    ensures result == raft_io_progress_valid_spec(*progress)
{
    progress.flushed <= progress.submitted && progress.submitted <= progress.accepted
}

// r[impl domain-primitives.raft-consensus.openraft-io-progress]
/// Return whether an I/O progress update is valid and cursor-monotone.
pub fn raft_io_progress_advance_admit(
    old_progress: &RaftIoProgressMetadata,
    new_progress: &RaftIoProgressMetadata,
) -> (result: bool)
    ensures result == raft_io_progress_advance_admit_spec(*old_progress, *new_progress)
{
    raft_io_progress_valid(old_progress)
        && raft_io_progress_valid(new_progress)
        && old_progress.flushed <= new_progress.flushed
        && old_progress.submitted <= new_progress.submitted
        && old_progress.accepted <= new_progress.accepted
}

// r[impl domain-primitives.raft-consensus.openraft-vote-log-io]
/// Return whether finite vote/term metadata is ordered before the modeled log write.
pub fn raft_vote_log_io_ordered(meta: &RaftVoteLogIoMetadata) -> (result: bool)
    ensures result == raft_vote_log_io_ordered_spec(*meta)
{
    meta.log_term <= meta.persisted_term
}

// r[impl domain-primitives.raft-consensus.openraft-effective-membership]
/// Return whether committed/effective membership epochs are ordered.
pub fn raft_effective_membership_valid(meta: &RaftEffectiveMembershipMetadata) -> (result: bool)
    ensures result == raft_effective_membership_valid_spec(*meta)
{
    meta.committed_epoch <= meta.effective_epoch
}

// r[impl domain-primitives.raft-consensus.openraft-effective-membership]
/// Roll back a conflicting uncommitted effective membership to the committed epoch.
pub fn raft_effective_membership_rollback(
    meta: &RaftEffectiveMembershipMetadata,
) -> (result: RaftEffectiveMembershipMetadata)
    ensures
        result == raft_effective_membership_rollback_spec(*meta),
        result.effective_epoch == meta.committed_epoch,
        result.committed_epoch == meta.committed_epoch,
{
    RaftEffectiveMembershipMetadata {
        committed_epoch: meta.committed_epoch,
        effective_epoch: meta.committed_epoch,
    }
}

// r[impl domain-primitives.raft-consensus.openraft-replication-session]
/// Return whether a replication progress update belongs to the current finite session.
pub fn raft_replication_session_matches(
    current: &RaftReplicationSessionMetadata,
    update: &RaftReplicationSessionMetadata,
) -> (result: bool)
    ensures result == raft_replication_session_matches_spec(*current, *update)
{
    current.leader_term == update.leader_term
        && current.leader_node == update.leader_node
        && current.membership_log_index == update.membership_log_index
}

// r[impl domain-primitives.raft-consensus.openraft-replication-session]
/// Return whether finite replication progress is current-session, monotone, and leader-log-bounded.
pub fn raft_replication_progress_update_admit(
    current: &RaftReplicationSessionMetadata,
    update_session: &RaftReplicationSessionMetadata,
    old_matched_index: u64,
    new_matched_index: u64,
    leader_last_log_index: u64,
) -> (result: bool)
    ensures result == raft_replication_progress_update_admit_spec(
        *current,
        *update_session,
        old_matched_index,
        new_matched_index,
        leader_last_log_index,
    )
{
    raft_replication_session_matches(current, update_session)
        && old_matched_index <= new_matched_index
        && new_matched_index <= leader_last_log_index
}

// r[impl domain-primitives.raft-consensus.openraft-read-linearizer]
/// Return whether local apply progress has caught up to the finite read-linearizer index.
pub fn raft_read_linearizer_ready(meta: &RaftReadLinearizerMetadata) -> (result: bool)
    ensures result == raft_read_linearizer_ready_spec(*meta)
{
    meta.read_log_index <= meta.last_applied_index
}

// r[impl domain-primitives.raft-consensus.openraft-log-pointers]
/// Return deterministic first-failure metadata for finite OpenRaft-inspired log pointers.
pub fn raft_openraft_log_pointers_admission_result(
    ptrs: &RaftOperationalLogPointers,
) -> (result: RaftOpenraftAdmissionResult)
    ensures result == raft_openraft_log_pointers_admission_result_spec(*ptrs)
{
    if ptrs.purged > ptrs.snapshot {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_PURGED_AFTER_SNAPSHOT }
    } else if ptrs.snapshot > ptrs.applied {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_SNAPSHOT_AFTER_APPLIED }
    } else if ptrs.applied > ptrs.committed {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_APPLIED_AFTER_COMMITTED }
    } else if ptrs.committed > ptrs.submitted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_COMMITTED_AFTER_SUBMITTED }
    } else if ptrs.flushed > ptrs.submitted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_FLUSHED_AFTER_SUBMITTED }
    } else if ptrs.submitted > ptrs.accepted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_SUBMITTED_AFTER_ACCEPTED }
    } else {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    }
}

// r[impl domain-primitives.raft-consensus.openraft-io-progress]
/// Return deterministic first-failure metadata for finite OpenRaft-inspired I/O progress.
pub fn raft_openraft_io_progress_admission_result(
    old_progress: &RaftIoProgressMetadata,
    new_progress: &RaftIoProgressMetadata,
) -> (result: RaftOpenraftAdmissionResult)
    ensures result == raft_openraft_io_progress_admission_result_spec(*old_progress, *new_progress)
{
    if !raft_io_progress_valid(old_progress) {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_OLD_PROGRESS_INVALID }
    } else if !raft_io_progress_valid(new_progress) {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_NEW_PROGRESS_INVALID }
    } else if old_progress.flushed > new_progress.flushed {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_FLUSHED_REGRESSED }
    } else if old_progress.submitted > new_progress.submitted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_SUBMITTED_REGRESSED }
    } else if old_progress.accepted > new_progress.accepted {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_ACCEPTED_REGRESSED }
    } else {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    }
}

// r[impl domain-primitives.raft-consensus.openraft-vote-log-io]
/// Return deterministic admission metadata for finite OpenRaft-inspired vote/log I/O ordering.
pub fn raft_openraft_vote_log_io_admission_result(
    meta: &RaftVoteLogIoMetadata,
) -> (result: RaftOpenraftAdmissionResult)
    ensures result == raft_openraft_vote_log_io_admission_result_spec(*meta)
{
    if raft_vote_log_io_ordered(meta) {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    } else {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_PRIOR_TERM_METADATA_REQUIRED }
    }
}

// r[impl domain-primitives.raft-consensus.openraft-effective-membership]
/// Return deterministic admission metadata for finite OpenRaft-inspired effective membership.
pub fn raft_openraft_effective_membership_admission_result(
    meta: &RaftEffectiveMembershipMetadata,
) -> (result: RaftOpenraftAdmissionResult)
    ensures result == raft_openraft_effective_membership_admission_result_spec(*meta)
{
    if raft_effective_membership_valid(meta) {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    } else {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_EFFECTIVE_BEFORE_COMMITTED }
    }
}

// r[impl domain-primitives.raft-consensus.openraft-replication-session]
/// Return deterministic first-failure metadata for finite OpenRaft-inspired replication progress.
pub fn raft_openraft_replication_progress_admission_result(
    current: &RaftReplicationSessionMetadata,
    update_session: &RaftReplicationSessionMetadata,
    old_matched_index: u64,
    new_matched_index: u64,
    leader_last_log_index: u64,
) -> (result: RaftOpenraftAdmissionResult)
    ensures result == raft_openraft_replication_progress_admission_result_spec(
        *current,
        *update_session,
        old_matched_index,
        new_matched_index,
        leader_last_log_index,
    )
{
    if !raft_replication_session_matches(current, update_session) {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_STALE_REPLICATION_SESSION }
    } else if old_matched_index > new_matched_index {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_MATCHED_INDEX_REGRESSED }
    } else if new_matched_index > leader_last_log_index {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_MATCHED_INDEX_AFTER_LEADER_LOG }
    } else {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    }
}

// r[impl domain-primitives.raft-consensus.openraft-read-linearizer]
/// Return deterministic readiness metadata for finite OpenRaft-inspired read-linearizers.
pub fn raft_openraft_read_linearizer_admission_result(
    meta: &RaftReadLinearizerMetadata,
) -> (result: RaftOpenraftAdmissionResult)
    ensures result == raft_openraft_read_linearizer_admission_result_spec(*meta)
{
    if raft_read_linearizer_ready(meta) {
        RaftOpenraftAdmissionResult { admitted: true, failing_check: RAFT_OPENRAFT_OK }
    } else {
        RaftOpenraftAdmissionResult { admitted: false, failing_check: RAFT_OPENRAFT_APPLY_CATCHUP_REQUIRED }
    }
}

// r[verify domain-primitives.raft-consensus.openraft-log-pointers]
/// Valid OpenRaft-inspired log pointers expose the core state-machine/commit ordering.
pub proof fn valid_log_pointers_order_applied_and_committed(ptrs: RaftOperationalLogPointers)
    requires raft_operational_log_pointers_valid_spec(ptrs)
    ensures
        ptrs.purged <= ptrs.snapshot,
        ptrs.snapshot <= ptrs.applied,
        ptrs.applied <= ptrs.committed,
        ptrs.committed <= ptrs.submitted,
        ptrs.flushed <= ptrs.accepted,
{}

// r[verify domain-primitives.raft-consensus.openraft-io-progress]
/// Admitted I/O progress updates never move any cursor backwards.
pub proof fn admitted_io_progress_update_is_monotone(
    old_progress: RaftIoProgressMetadata,
    new_progress: RaftIoProgressMetadata,
)
    requires raft_io_progress_advance_admit_spec(old_progress, new_progress)
    ensures
        old_progress.flushed <= new_progress.flushed,
        old_progress.submitted <= new_progress.submitted,
        old_progress.accepted <= new_progress.accepted,
{}

// r[verify domain-primitives.raft-consensus.openraft-vote-log-io]
/// Admitted finite log I/O has persisted term metadata at least as new as the log term.
pub proof fn admitted_log_io_has_prior_term_metadata(meta: RaftVoteLogIoMetadata)
    requires raft_vote_log_io_ordered_spec(meta)
    ensures meta.log_term <= meta.persisted_term
{}

// r[verify domain-primitives.raft-consensus.openraft-effective-membership]
/// Effective-membership rollback restores effective membership to the committed epoch.
pub proof fn membership_rollback_restores_committed_epoch(meta: RaftEffectiveMembershipMetadata)
    ensures
        raft_effective_membership_rollback_spec(meta).committed_epoch == meta.committed_epoch,
        raft_effective_membership_rollback_spec(meta).effective_epoch == meta.committed_epoch,
        raft_effective_membership_valid_spec(raft_effective_membership_rollback_spec(meta)),
{}

// r[verify domain-primitives.raft-consensus.openraft-replication-session]
/// Admitted replication progress updates are current-session and bounded by the leader log.
pub proof fn admitted_replication_progress_is_current_and_bounded(
    current: RaftReplicationSessionMetadata,
    update_session: RaftReplicationSessionMetadata,
    old_matched_index: u64,
    new_matched_index: u64,
    leader_last_log_index: u64,
)
    requires raft_replication_progress_update_admit_spec(
        current,
        update_session,
        old_matched_index,
        new_matched_index,
        leader_last_log_index,
    )
    ensures
        raft_replication_session_matches_spec(current, update_session),
        old_matched_index <= new_matched_index,
        new_matched_index <= leader_last_log_index,
{}

// r[verify domain-primitives.raft-consensus.openraft-read-linearizer]
/// Ready finite read-linearizers expose that local apply has caught up to the read index.
pub proof fn ready_read_linearizer_has_applied_read_index(meta: RaftReadLinearizerMetadata)
    requires raft_read_linearizer_ready_spec(meta)
    ensures meta.read_log_index <= meta.last_applied_index
{}

// r[verify domain-primitives.raft-consensus.openraft-log-pointers]
/// OK OpenRaft-inspired log-pointer admission is exactly the existing pointer lattice predicate.
pub proof fn openraft_log_pointer_ok_implies_valid(ptrs: RaftOperationalLogPointers)
    requires raft_openraft_log_pointers_admission_result_spec(ptrs).admitted
    ensures raft_operational_log_pointers_valid_spec(ptrs)
{}

// r[verify domain-primitives.raft-consensus.openraft-log-pointers]
/// A purged-after-snapshot pointer violation is reported before later pointer checks.
pub proof fn openraft_log_pointer_purged_failure_is_first(ptrs: RaftOperationalLogPointers)
    requires ptrs.purged > ptrs.snapshot
    ensures
        !raft_openraft_log_pointers_admission_result_spec(ptrs).admitted,
        raft_openraft_log_pointers_admission_result_spec(ptrs).failing_check == RAFT_OPENRAFT_PURGED_AFTER_SNAPSHOT,
{}

// r[verify domain-primitives.raft-consensus.openraft-io-progress]
/// OK OpenRaft-inspired I/O progress admission is exactly the existing monotone-update predicate.
pub proof fn openraft_io_progress_ok_implies_admitted(
    old_progress: RaftIoProgressMetadata,
    new_progress: RaftIoProgressMetadata,
)
    requires raft_openraft_io_progress_admission_result_spec(old_progress, new_progress).admitted
    ensures raft_io_progress_advance_admit_spec(old_progress, new_progress)
{}

// r[verify domain-primitives.raft-consensus.openraft-io-progress]
/// Invalid old I/O progress is reported before update regression checks.
pub proof fn openraft_io_progress_invalid_old_failure_is_first(
    old_progress: RaftIoProgressMetadata,
    new_progress: RaftIoProgressMetadata,
)
    requires !raft_io_progress_valid_spec(old_progress)
    ensures
        !raft_openraft_io_progress_admission_result_spec(old_progress, new_progress).admitted,
        raft_openraft_io_progress_admission_result_spec(old_progress, new_progress).failing_check == RAFT_OPENRAFT_OLD_PROGRESS_INVALID,
{}

// r[verify domain-primitives.raft-consensus.openraft-vote-log-io]
/// OK vote/log I/O admission is exactly the existing prior-term ordering predicate.
pub proof fn openraft_vote_log_io_ok_implies_ordered(meta: RaftVoteLogIoMetadata)
    requires raft_openraft_vote_log_io_admission_result_spec(meta).admitted
    ensures raft_vote_log_io_ordered_spec(meta)
{}

// r[verify domain-primitives.raft-consensus.openraft-effective-membership]
/// OK effective-membership admission is exactly the existing committed/effective epoch order.
pub proof fn openraft_effective_membership_ok_implies_valid(meta: RaftEffectiveMembershipMetadata)
    requires raft_openraft_effective_membership_admission_result_spec(meta).admitted
    ensures raft_effective_membership_valid_spec(meta)
{}

// r[verify domain-primitives.raft-consensus.openraft-replication-session]
/// OK replication-progress admission is exactly the existing current-session bounded-update predicate.
pub proof fn openraft_replication_progress_ok_implies_admitted(
    current: RaftReplicationSessionMetadata,
    update_session: RaftReplicationSessionMetadata,
    old_matched_index: u64,
    new_matched_index: u64,
    leader_last_log_index: u64,
)
    requires raft_openraft_replication_progress_admission_result_spec(
        current,
        update_session,
        old_matched_index,
        new_matched_index,
        leader_last_log_index,
    ).admitted
    ensures raft_replication_progress_update_admit_spec(
        current,
        update_session,
        old_matched_index,
        new_matched_index,
        leader_last_log_index,
    )
{}

// r[verify domain-primitives.raft-consensus.openraft-replication-session]
/// Stale replication sessions are rejected before matched-index checks.
pub proof fn openraft_replication_stale_session_failure_is_first(
    current: RaftReplicationSessionMetadata,
    update_session: RaftReplicationSessionMetadata,
    old_matched_index: u64,
    new_matched_index: u64,
    leader_last_log_index: u64,
)
    requires !raft_replication_session_matches_spec(current, update_session)
    ensures
        !raft_openraft_replication_progress_admission_result_spec(
            current,
            update_session,
            old_matched_index,
            new_matched_index,
            leader_last_log_index,
        ).admitted,
        raft_openraft_replication_progress_admission_result_spec(
            current,
            update_session,
            old_matched_index,
            new_matched_index,
            leader_last_log_index,
        ).failing_check == RAFT_OPENRAFT_STALE_REPLICATION_SESSION,
{}

// r[verify domain-primitives.raft-consensus.openraft-read-linearizer]
/// OK read-linearizer admission is exactly the existing local-apply catch-up predicate.
pub proof fn openraft_read_linearizer_ok_implies_ready(meta: RaftReadLinearizerMetadata)
    requires raft_openraft_read_linearizer_admission_result_spec(meta).admitted
    ensures raft_read_linearizer_ready_spec(meta)
{}

// r[depends domain-primitives.raft-consensus.membership]
/// Spec: finite membership metadata is bounded to a modeled one-based node universe.
pub open spec fn raft_membership_config_valid_spec(config: RaftMembershipConfig) -> bool {
    config.max_node_id > 0
        && config.old_voter_count > 0
        && config.old_voter_count <= config.max_node_id
        && config.new_voter_count > 0
        && config.new_voter_count <= config.max_node_id
        && (!config.is_joint ==> config.new_voter_count == config.old_voter_count)
}

// r[depends domain-primitives.raft-consensus.membership]
/// Spec: configuration epochs never move backwards, and same-epoch changes must be identical.
pub open spec fn raft_membership_epoch_advance_admit_spec(
    old_config: RaftMembershipConfig,
    new_config: RaftMembershipConfig,
) -> bool {
    raft_membership_config_valid_spec(old_config)
        && raft_membership_config_valid_spec(new_config)
        && old_config.epoch <= new_config.epoch
        && (old_config.epoch == new_config.epoch ==> (
            old_config.old_voter_count == new_config.old_voter_count
                && old_config.new_voter_count == new_config.new_voter_count
                && old_config.is_joint == new_config.is_joint
                && old_config.max_node_id == new_config.max_node_id
        ))
}

// r[depends domain-primitives.raft-consensus.membership]
/// Spec: joint consensus admits only proposals with quorum in both old and new configurations.
pub open spec fn raft_joint_quorum_admit_spec(
    config: RaftMembershipConfig,
    old_ack_count: u64,
    new_ack_count: u64,
) -> bool {
    raft_membership_config_valid_spec(config)
        && if config.is_joint {
            is_quorum_spec(old_ack_count, config.old_voter_count)
                && is_quorum_spec(new_ack_count, config.new_voter_count)
        } else {
            is_quorum_spec(old_ack_count, config.old_voter_count)
        }
}

// r[impl domain-primitives.raft-consensus.membership]
/// Return whether finite membership metadata is bounded to the modeled node universe.
pub fn raft_membership_config_valid(config: &RaftMembershipConfig) -> (result: bool)
    ensures result == raft_membership_config_valid_spec(*config)
{
    config.max_node_id > 0
        && config.old_voter_count > 0
        && config.old_voter_count <= config.max_node_id
        && config.new_voter_count > 0
        && config.new_voter_count <= config.max_node_id
        && (config.is_joint || config.new_voter_count == config.old_voter_count)
}

// r[impl domain-primitives.raft-consensus.membership]
/// Return whether a membership epoch transition is monotone and stable at equal epochs.
pub fn raft_membership_epoch_advance_admit(
    old_config: &RaftMembershipConfig,
    new_config: &RaftMembershipConfig,
) -> (result: bool)
    ensures result == raft_membership_epoch_advance_admit_spec(*old_config, *new_config)
{
    raft_membership_config_valid(old_config)
        && raft_membership_config_valid(new_config)
        && old_config.epoch <= new_config.epoch
        && (old_config.epoch != new_config.epoch
            || (old_config.old_voter_count == new_config.old_voter_count
                && old_config.new_voter_count == new_config.new_voter_count
                && old_config.is_joint == new_config.is_joint
                && old_config.max_node_id == new_config.max_node_id))
}

// r[impl domain-primitives.raft-consensus.membership]
/// Return whether finite quorum metadata admits a normal or joint-consensus proposal.
pub fn raft_joint_quorum_admit(
    config: &RaftMembershipConfig,
    old_ack_count: u64,
    new_ack_count: u64,
) -> (result: bool)
    ensures result == raft_joint_quorum_admit_spec(*config, old_ack_count, new_ack_count)
{
    if !raft_membership_config_valid(config) {
        return false;
    }
    if config.is_joint {
        is_quorum(old_ack_count, config.old_voter_count)
            && is_quorum(new_ack_count, config.new_voter_count)
    } else {
        is_quorum(old_ack_count, config.old_voter_count)
    }
}

// r[verify domain-primitives.raft-consensus.membership]
/// Valid finite membership metadata has non-empty old and new voter sets bounded by the universe.
pub proof fn valid_membership_has_bounded_nonempty_voters(config: RaftMembershipConfig)
    requires raft_membership_config_valid_spec(config)
    ensures
        0 < config.old_voter_count <= config.max_node_id,
        0 < config.new_voter_count <= config.max_node_id,
{}

// r[verify domain-primitives.raft-consensus.membership]
/// Admitted membership epoch changes never move the configuration epoch backwards.
pub proof fn admitted_membership_epoch_is_monotone(
    old_config: RaftMembershipConfig,
    new_config: RaftMembershipConfig,
)
    requires raft_membership_epoch_advance_admit_spec(old_config, new_config)
    ensures old_config.epoch <= new_config.epoch
{}

// r[verify domain-primitives.raft-consensus.membership]
/// Admitted joint-consensus quorums have quorum support in both old and new configurations.
pub proof fn admitted_joint_quorum_has_both_majorities(
    config: RaftMembershipConfig,
    old_ack_count: u64,
    new_ack_count: u64,
)
    requires
        raft_joint_quorum_admit_spec(config, old_ack_count, new_ack_count),
        config.is_joint,
    ensures
        is_quorum_spec(old_ack_count, config.old_voter_count),
        is_quorum_spec(new_ack_count, config.new_voter_count),
{}

// r[verify domain-primitives.raft-consensus.membership]
/// Any two admitted joint-consensus quorums intersect in both old and new configurations by quorum arithmetic.
pub proof fn admitted_joint_quorums_intersect(
    config: RaftMembershipConfig,
    old_ack_a: u64,
    new_ack_a: u64,
    old_ack_b: u64,
    new_ack_b: u64,
)
    requires
        raft_membership_config_valid_spec(config),
        config.is_joint,
        raft_joint_quorum_admit_spec(config, old_ack_a, new_ack_a),
        raft_joint_quorum_admit_spec(config, old_ack_b, new_ack_b),
    ensures
        old_ack_a as int + old_ack_b as int > config.old_voter_count as int,
        new_ack_a as int + new_ack_b as int > config.new_voter_count as int,
{
    prove_quorum_intersection(old_ack_a, old_ack_b, config.old_voter_count);
    prove_quorum_intersection(new_ack_a, new_ack_b, config.new_voter_count);
}

// r[depends domain-primitives.raft-consensus.snapshot]
/// Spec: finite snapshot metadata has a coherent empty/non-empty boundary.
pub open spec fn raft_snapshot_metadata_valid_spec(snapshot: RaftSnapshotMetadata) -> bool {
    snapshot.last_included_index == 0 ==> snapshot.last_included_term == 0
}

// r[depends domain-primitives.raft-consensus.snapshot]
/// Spec: snapshot boundary matches the compacted log prefix boundary.
pub open spec fn raft_snapshot_matches_prefix_spec(
    log: Seq<RaftIndexedLogEntry>,
    snapshot: RaftSnapshotMetadata,
) -> bool {
    raft_snapshot_metadata_valid_spec(snapshot)
        && if snapshot.last_included_index == 0 {
            snapshot.last_included_term == 0
        } else {
            snapshot.last_included_index as int <= log.len()
                && log[snapshot.last_included_index as int - 1].index == snapshot.last_included_index as u64
                && log[snapshot.last_included_index as int - 1].term == snapshot.last_included_term
        }
}

// r[depends domain-primitives.raft-consensus.snapshot]
/// Spec: prefix compaction drops the included prefix and retains the suffix after the snapshot boundary.
pub open spec fn raft_snapshot_compact_log_spec(
    log: Seq<RaftIndexedLogEntry>,
    snapshot: RaftSnapshotMetadata,
) -> Seq<RaftIndexedLogEntry>
    recommends snapshot.last_included_index as int <= log.len()
{
    log.subrange(snapshot.last_included_index as int, log.len() as int)
}

// r[depends domain-primitives.raft-consensus.install-snapshot]
/// Spec: InstallSnapshot admission accepts only non-stale term and newer, prefix-compatible snapshots.
pub open spec fn raft_install_snapshot_admit_spec(
    current_term: u64,
    leader_term: u64,
    current_snapshot_index: usize,
    log: Seq<RaftIndexedLogEntry>,
    snapshot: RaftSnapshotMetadata,
) -> bool {
    current_term <= leader_term
        && current_snapshot_index < snapshot.last_included_index
        && raft_snapshot_matches_prefix_spec(log, snapshot)
}

// r[impl domain-primitives.raft-consensus.snapshot]
/// Return whether finite snapshot metadata has a coherent empty/non-empty boundary.
pub fn raft_snapshot_metadata_valid(snapshot: &RaftSnapshotMetadata) -> (result: bool)
    ensures result == raft_snapshot_metadata_valid_spec(*snapshot)
{
    snapshot.last_included_index != 0 || snapshot.last_included_term == 0
}

// r[impl domain-primitives.raft-consensus.snapshot]
/// Return whether a snapshot boundary matches the finite log prefix boundary.
pub fn raft_snapshot_matches_prefix(
    log: &[RaftIndexedLogEntry],
    snapshot: &RaftSnapshotMetadata,
) -> (result: bool)
    ensures result == raft_snapshot_matches_prefix_spec(log@, *snapshot)
{
    if !raft_snapshot_metadata_valid(snapshot) {
        return false;
    }
    if snapshot.last_included_index == 0 {
        return snapshot.last_included_term == 0;
    }
    if snapshot.last_included_index > log.len() {
        return false;
    }
    let boundary = snapshot.last_included_index - 1;
    log[boundary].index == snapshot.last_included_index as u64
        && log[boundary].term == snapshot.last_included_term
}

// r[impl domain-primitives.raft-consensus.install-snapshot]
/// Return whether finite InstallSnapshot metadata is term-valid, newer, and prefix-compatible.
pub fn raft_install_snapshot_admit(
    current_term: u64,
    leader_term: u64,
    current_snapshot_index: usize,
    log: &[RaftIndexedLogEntry],
    snapshot: &RaftSnapshotMetadata,
) -> (result: bool)
    ensures result == raft_install_snapshot_admit_spec(
        current_term,
        leader_term,
        current_snapshot_index,
        log@,
        *snapshot,
    )
{
    current_term <= leader_term
        && current_snapshot_index < snapshot.last_included_index
        && raft_snapshot_matches_prefix(log, snapshot)
}

// r[verify domain-primitives.raft-consensus.snapshot]
/// Matched non-empty snapshots identify an in-bounds log boundary with the same term.
pub proof fn matched_snapshot_boundary_is_in_bounds(
    log: Seq<RaftIndexedLogEntry>,
    snapshot: RaftSnapshotMetadata,
)
    requires
        raft_snapshot_matches_prefix_spec(log, snapshot),
        snapshot.last_included_index > 0,
    ensures
        snapshot.last_included_index as int <= log.len(),
        log[snapshot.last_included_index as int - 1].term == snapshot.last_included_term,
{}

// r[verify domain-primitives.raft-consensus.snapshot]
/// Prefix compaction preserves the suffix length after the snapshot boundary.
pub proof fn snapshot_compaction_suffix_length(
    log: Seq<RaftIndexedLogEntry>,
    snapshot: RaftSnapshotMetadata,
)
    requires snapshot.last_included_index as int <= log.len()
    ensures raft_snapshot_compact_log_spec(log, snapshot).len()
        == log.len() - snapshot.last_included_index as int
{}

// r[verify domain-primitives.raft-consensus.install-snapshot]
/// Admitted InstallSnapshot requests are non-stale and strictly newer than the current boundary.
pub proof fn admitted_install_snapshot_is_newer(
    current_term: u64,
    leader_term: u64,
    current_snapshot_index: usize,
    log: Seq<RaftIndexedLogEntry>,
    snapshot: RaftSnapshotMetadata,
)
    requires raft_install_snapshot_admit_spec(current_term, leader_term, current_snapshot_index, log, snapshot)
    ensures
        current_term <= leader_term,
        current_snapshot_index < snapshot.last_included_index,
        raft_snapshot_matches_prefix_spec(log, snapshot),
{}

// r[depends domain-primitives.raft-consensus.persistent-state]
/// Spec: finite persisted metadata is bounded to the modeled node universe.
pub open spec fn raft_persistent_metadata_valid_spec(meta: RaftPersistentMetadata, max_node: u64) -> bool {
    meta.voted_for <= max_node
}

// r[depends domain-primitives.raft-consensus.persistent-state]
/// Spec: observing a higher term updates the persisted term and clears the recorded vote.
pub open spec fn raft_persistent_observe_term_spec(
    meta: RaftPersistentMetadata,
    observed_term: u64,
) -> RaftPersistentMetadata {
    if observed_term > meta.current_term {
        RaftPersistentMetadata { current_term: observed_term, voted_for: 0 }
    } else {
        meta
    }
}

// r[depends domain-primitives.raft-consensus.volatile-state]
/// Spec: finite volatile metadata keeps applied, committed, and log indexes ordered.
pub open spec fn raft_volatile_metadata_valid_spec(meta: RaftVolatileMetadata) -> bool {
    meta.last_applied <= meta.commit_index && meta.commit_index <= meta.last_log_index
}

// r[depends domain-primitives.raft-consensus.volatile-state]
/// Spec: a commit-index advance is admitted only within the local log and never backwards.
pub open spec fn raft_commit_advance_admit_spec(meta: RaftVolatileMetadata, new_commit: u64) -> bool {
    raft_volatile_metadata_valid_spec(meta)
        && meta.commit_index <= new_commit
        && new_commit <= meta.last_log_index
}

// r[impl domain-primitives.raft-consensus.persistent-state]
/// Return whether finite persisted Raft metadata is inside the modeled node bound.
pub fn raft_persistent_metadata_valid(meta: &RaftPersistentMetadata, max_node: u64) -> (result: bool)
    ensures result == raft_persistent_metadata_valid_spec(*meta, max_node)
{
    meta.voted_for <= max_node
}

// r[impl domain-primitives.raft-consensus.persistent-state]
/// Update finite persisted metadata for an observed term, clearing votes only on term advance.
pub fn raft_persistent_observe_term(
    meta: &RaftPersistentMetadata,
    observed_term: u64,
) -> (result: RaftPersistentMetadata)
    ensures
        result == raft_persistent_observe_term_spec(*meta, observed_term),
        result.current_term >= meta.current_term,
        observed_term > meta.current_term ==> result.voted_for == 0,
{
    if observed_term > meta.current_term {
        RaftPersistentMetadata { current_term: observed_term, voted_for: 0 }
    } else {
        *meta
    }
}

// r[impl domain-primitives.raft-consensus.volatile-state]
/// Return whether finite volatile Raft metadata has ordered applied/commit/log indexes.
pub fn raft_volatile_metadata_valid(meta: &RaftVolatileMetadata) -> (result: bool)
    ensures result == raft_volatile_metadata_valid_spec(*meta)
{
    meta.last_applied <= meta.commit_index && meta.commit_index <= meta.last_log_index
}

// r[impl domain-primitives.raft-consensus.volatile-state]
/// Return whether a finite commit-index advance is inside the local log and monotone.
pub fn raft_commit_advance_admit(meta: &RaftVolatileMetadata, new_commit: u64) -> (result: bool)
    ensures result == raft_commit_advance_admit_spec(*meta, new_commit)
{
    raft_volatile_metadata_valid(meta) && meta.commit_index <= new_commit && new_commit <= meta.last_log_index
}

// r[verify domain-primitives.raft-consensus.persistent-state]
/// Observing a higher term keeps persisted terms monotone and clears the prior vote.
pub proof fn persistent_observe_higher_term_clears_vote(meta: RaftPersistentMetadata, observed_term: u64)
    requires observed_term > meta.current_term
    ensures
        raft_persistent_observe_term_spec(meta, observed_term).current_term == observed_term,
        raft_persistent_observe_term_spec(meta, observed_term).voted_for == 0,
        raft_persistent_observe_term_spec(meta, observed_term).current_term >= meta.current_term,
{}

// r[verify domain-primitives.raft-consensus.persistent-state]
/// Observing a non-higher term leaves persisted metadata unchanged.
pub proof fn persistent_observe_non_higher_term_unchanged(meta: RaftPersistentMetadata, observed_term: u64)
    requires observed_term <= meta.current_term
    ensures raft_persistent_observe_term_spec(meta, observed_term) == meta
{}

// r[verify domain-primitives.raft-consensus.volatile-state]
/// Admitted commit advances never move commit index backwards and never exceed the local log.
pub proof fn admitted_commit_advance_is_ordered(meta: RaftVolatileMetadata, new_commit: u64)
    requires raft_commit_advance_admit_spec(meta, new_commit)
    ensures
        meta.commit_index <= new_commit,
        new_commit <= meta.last_log_index,
        meta.last_applied <= new_commit,
{}

// r[depends domain-primitives.raft-consensus.append-entries]
/// Spec: AppendEntries conflict metadata is bounded by the finite follower-log length.
pub open spec fn raft_append_conflict_metadata_valid_spec(meta: RaftAppendConflictMetadata) -> bool {
    meta.conflict_index <= meta.follower_log_len
        && (meta.conflict_index == 0 ==> meta.conflict_term == 0)
}

// r[depends domain-primitives.raft-consensus.follower-log-update]
/// Spec: a finite follower log update truncates after the admitted previous-log prefix and appends entries.
pub open spec fn raft_follower_log_update_spec(
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    entries: Seq<LogEntry>,
) -> Seq<LogEntry>
    recommends prev_log_index as int <= follower_log.len()
{
    follower_log.subrange(0, prev_log_index as int) + entries
}

// r[depends domain-primitives.raft-consensus.follower-log-update]
/// Spec: finite follower log-update metadata admits rejection/no-mutation or accepted prefix-plus-append length.
pub open spec fn raft_follower_log_update_metadata_valid_spec(
    meta: RaftFollowerLogUpdateMetadata,
    follower_log_len: u64,
) -> bool {
    if meta.rejected {
        meta.successor_len == follower_log_len
    } else {
        meta.prev_log_index <= follower_log_len
            && meta.successor_len >= meta.prev_log_index
            && meta.successor_len - meta.prev_log_index == meta.appended_len
    }
}

// r[impl domain-primitives.raft-consensus.follower-log-update]
/// Return whether finite follower log-update metadata is internally admitted.
/// This checks only finite prefix/length/no-mutation metadata; it does not claim storage durability.
pub fn raft_follower_log_update_metadata_valid(
    meta: &RaftFollowerLogUpdateMetadata,
    follower_log_len: u64,
) -> (result: bool)
    ensures result == raft_follower_log_update_metadata_valid_spec(*meta, follower_log_len)
{
    if meta.rejected {
        meta.successor_len == follower_log_len
    } else if meta.prev_log_index > follower_log_len {
        false
    } else if meta.successor_len < meta.prev_log_index {
        false
    } else {
        meta.successor_len - meta.prev_log_index == meta.appended_len
    }
}

// r[impl domain-primitives.raft-consensus.append-entries]
/// Return whether finite AppendEntries conflict metadata is internally bounded.
pub fn raft_append_conflict_metadata_valid(meta: &RaftAppendConflictMetadata) -> (result: bool)
    ensures result == raft_append_conflict_metadata_valid_spec(*meta)
{
    meta.conflict_index <= meta.follower_log_len
        && (meta.conflict_index != 0 || meta.conflict_term == 0)
}

// r[verify domain-primitives.raft-consensus.append-entries]
/// Concrete conflict indexes are always inside the follower log metadata.
pub proof fn concrete_append_conflict_is_bounded(meta: RaftAppendConflictMetadata)
    requires
        raft_append_conflict_metadata_valid_spec(meta),
        meta.conflict_index > 0,
    ensures
        meta.conflict_index <= meta.follower_log_len,
        meta.conflict_term == meta.conflict_term,
{}

// r[verify domain-primitives.raft-consensus.follower-log-update]
/// An admitted follower update preserves every entry before the previous-log index.
pub proof fn follower_update_preserves_admitted_prefix(
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    entries: Seq<LogEntry>,
    i: int,
)
    requires
        prev_log_index as int <= follower_log.len(),
        0 <= i < prev_log_index as int,
    ensures raft_follower_log_update_spec(follower_log, prev_log_index, entries)[i] == follower_log[i]
{}

// r[verify domain-primitives.raft-consensus.follower-log-update]
/// A follower update length is the retained prefix plus the appended entry count.
pub proof fn follower_update_length_matches_prefix_plus_entries(
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    entries: Seq<LogEntry>,
)
    requires prev_log_index as int <= follower_log.len()
    ensures raft_follower_log_update_spec(follower_log, prev_log_index, entries).len()
        == prev_log_index as int + entries.len()
{}

// r[verify domain-primitives.raft-consensus.follower-log-update]
/// Rejected follower updates carry no-mutation length metadata.
pub proof fn rejected_follower_update_preserves_log_length(
    meta: RaftFollowerLogUpdateMetadata,
    follower_log_len: u64,
)
    requires
        raft_follower_log_update_metadata_valid_spec(meta, follower_log_len),
        meta.rejected,
    ensures meta.successor_len == follower_log_len
{}

// r[verify domain-primitives.raft-consensus.follower-log-update]
/// Accepted follower updates retain a bounded prefix and append exactly the recorded length.
pub proof fn accepted_follower_update_length_is_prefix_plus_append(
    meta: RaftFollowerLogUpdateMetadata,
    follower_log_len: u64,
)
    requires
        raft_follower_log_update_metadata_valid_spec(meta, follower_log_len),
        !meta.rejected,
    ensures
        meta.prev_log_index <= follower_log_len,
        meta.successor_len >= meta.prev_log_index,
        meta.successor_len - meta.prev_log_index == meta.appended_len,
{}

// r[depends domain-primitives.raft-consensus.log-matching]
/// Spec: Raft log matching at a one-based index requires equal terms there and matching prior terms.
pub open spec fn raft_log_matching_at_spec(a: Seq<LogEntry>, b: Seq<LogEntry>, index: u64) -> bool {
    index > 0
        && index as int <= a.len()
        && index as int <= b.len()
        && a[(index - 1) as int].term == b[(index - 1) as int].term
        && logs_match_prefix_spec(a, b, index as int)
}

// r[impl domain-primitives.raft-consensus.log-matching]
/// Return whether two finite Raft logs match through the given one-based index.
pub fn raft_log_matching_at(a: &[LogEntry], b: &[LogEntry], index: usize) -> (result: bool)
    ensures result == raft_log_matching_at_spec(a@, b@, index as u64)
{
    if index == 0 || index > a.len() || index > b.len() {
        return false;
    }
    if a[index - 1].term != b[index - 1].term {
        return false;
    }
    logs_match_prefix(a, b, index)
}

// r[verify domain-primitives.raft-consensus.log-matching]
/// Log matching at an index implies all prior terms match as a prefix.
pub proof fn log_matching_implies_prefix_match(a: Seq<LogEntry>, b: Seq<LogEntry>, index: u64)
    requires raft_log_matching_at_spec(a, b, index)
    ensures logs_match_prefix_spec(a, b, index as int)
{}

// r[verify domain-primitives.raft-consensus.log-matching]
/// Follower updates preserve the admitted matching prefix before appended entries.
pub proof fn follower_update_preserves_matching_prefix(
    leader_log: Seq<LogEntry>,
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    entries: Seq<LogEntry>,
)
    requires
        prev_log_index as int <= follower_log.len(),
        logs_match_prefix_spec(leader_log, follower_log, prev_log_index as int),
    ensures logs_match_prefix_spec(
        leader_log,
        raft_follower_log_update_spec(follower_log, prev_log_index, entries),
        prev_log_index as int,
    )
{
    assert forall|i: int| 0 <= i < prev_log_index as int implies
        leader_log[i].term == raft_follower_log_update_spec(follower_log, prev_log_index, entries)[i].term by {
            assert(leader_log[i].term == follower_log[i].term);
            assert(raft_follower_log_update_spec(follower_log, prev_log_index, entries)[i] == follower_log[i]);
        }
}

// r[depends domain-primitives.raft-consensus.commit]
/// Spec: quorum commit metadata admits an index only when enough replicas match it.
pub open spec fn raft_quorum_commit_admit_spec(
    match_count: u64,
    cluster_size: u64,
    commit_index: u64,
    candidate_index: u64,
    last_log_index: u64,
) -> bool
    recommends cluster_size > 0
{
    is_quorum_spec(match_count, cluster_size)
        && commit_index <= candidate_index
        && candidate_index <= last_log_index
}

// r[depends domain-primitives.raft-consensus.commit]
/// Spec: admitted quorum commit advances to the candidate index and otherwise stays unchanged.
pub open spec fn raft_commit_index_update_spec(
    match_count: u64,
    cluster_size: u64,
    commit_index: u64,
    candidate_index: u64,
    last_log_index: u64,
) -> u64
    recommends cluster_size > 0
{
    if raft_quorum_commit_admit_spec(match_count, cluster_size, commit_index, candidate_index, last_log_index) {
        candidate_index
    } else {
        commit_index
    }
}

// r[impl domain-primitives.raft-consensus.commit]
/// Return whether a finite candidate commit index has quorum support and local-log bounds.
pub fn raft_quorum_commit_admit(
    match_count: u64,
    cluster_size: u64,
    commit_index: u64,
    candidate_index: u64,
    last_log_index: u64,
) -> (result: bool)
    requires cluster_size > 0
    ensures result == raft_quorum_commit_admit_spec(
        match_count,
        cluster_size,
        commit_index,
        candidate_index,
        last_log_index,
    )
{
    is_quorum(match_count, cluster_size) && commit_index <= candidate_index && candidate_index <= last_log_index
}

// r[impl domain-primitives.raft-consensus.commit]
/// Return the finite commit index after applying quorum-commit admission.
pub fn raft_commit_index_update(
    match_count: u64,
    cluster_size: u64,
    commit_index: u64,
    candidate_index: u64,
    last_log_index: u64,
) -> (result: u64)
    requires cluster_size > 0
    ensures
        result == raft_commit_index_update_spec(
            match_count,
            cluster_size,
            commit_index,
            candidate_index,
            last_log_index,
        ),
        result >= commit_index,
        result <= last_log_index || commit_index > last_log_index,
{
    if raft_quorum_commit_admit(match_count, cluster_size, commit_index, candidate_index, last_log_index) {
        candidate_index
    } else {
        commit_index
    }
}

// r[verify domain-primitives.raft-consensus.commit]
/// Admitted quorum commits are monotone and bounded by the local log.
pub proof fn admitted_quorum_commit_is_ordered(
    match_count: u64,
    cluster_size: u64,
    commit_index: u64,
    candidate_index: u64,
    last_log_index: u64,
)
    requires
        cluster_size > 0,
        raft_quorum_commit_admit_spec(match_count, cluster_size, commit_index, candidate_index, last_log_index),
    ensures
        commit_index <= candidate_index,
        candidate_index <= last_log_index,
        is_quorum_spec(match_count, cluster_size),
{}

// r[verify domain-primitives.raft-consensus.commit]
/// Rejected quorum commits leave the commit index unchanged.
pub proof fn rejected_quorum_commit_stays_put(
    match_count: u64,
    cluster_size: u64,
    commit_index: u64,
    candidate_index: u64,
    last_log_index: u64,
)
    requires
        cluster_size > 0,
        !raft_quorum_commit_admit_spec(match_count, cluster_size, commit_index, candidate_index, last_log_index),
    ensures raft_commit_index_update_spec(match_count, cluster_size, commit_index, candidate_index, last_log_index) == commit_index
{}

// r[depends domain-primitives.raft-consensus.leader-completeness]
/// Spec: a later leader preserves the committed entry at the same finite log position.
pub open spec fn raft_leader_contains_committed_entry_spec(
    committed_log: Seq<RaftIndexedLogEntry>,
    leader_log: Seq<RaftIndexedLogEntry>,
    committed_pos: int,
) -> bool {
    0 <= committed_pos
        && committed_pos < committed_log.len()
        && committed_pos < leader_log.len()
        && leader_log[committed_pos].index == committed_log[committed_pos].index
        && leader_log[committed_pos].term == committed_log[committed_pos].term
        && leader_log[committed_pos].command == committed_log[committed_pos].command
}

// r[impl domain-primitives.raft-consensus.leader-completeness]
/// Return whether a later leader log contains the finite committed entry at `committed_pos`.
pub fn raft_leader_contains_committed_entry(
    committed_log: &[RaftIndexedLogEntry],
    leader_log: &[RaftIndexedLogEntry],
    committed_pos: usize,
) -> (result: bool)
    ensures result == raft_leader_contains_committed_entry_spec(
        committed_log@,
        leader_log@,
        committed_pos as int,
    )
{
    committed_pos < committed_log.len()
        && committed_pos < leader_log.len()
        && leader_log[committed_pos].index == committed_log[committed_pos].index
        && leader_log[committed_pos].term == committed_log[committed_pos].term
        && leader_log[committed_pos].command == committed_log[committed_pos].command
}

// r[verify domain-primitives.raft-consensus.leader-completeness]
/// Leader-completeness admission exposes same index, term, and command for the committed position.
pub proof fn admitted_leader_completeness_preserves_entry(
    committed_log: Seq<RaftIndexedLogEntry>,
    leader_log: Seq<RaftIndexedLogEntry>,
    committed_pos: int,
)
    requires raft_leader_contains_committed_entry_spec(committed_log, leader_log, committed_pos)
    ensures
        leader_log[committed_pos].index == committed_log[committed_pos].index,
        leader_log[committed_pos].term == committed_log[committed_pos].term,
        leader_log[committed_pos].command == committed_log[committed_pos].command,
{}

// r[depends domain-primitives.raft-consensus.state-machine-safety]
/// Spec: applied prefixes agree on the command at a shared applied position.
pub open spec fn raft_state_machine_safety_at_spec(
    a: Seq<RaftIndexedLogEntry>,
    b: Seq<RaftIndexedLogEntry>,
    applied_a: int,
    applied_b: int,
    pos: int,
) -> bool {
    0 <= pos
        && pos < applied_a
        && pos < applied_b
        && pos < a.len()
        && pos < b.len()
        && a[pos].command == b[pos].command
}

// r[impl domain-primitives.raft-consensus.state-machine-safety]
/// Return whether two finite applied prefixes agree on a command at `pos`.
pub fn raft_state_machine_safety_at(
    a: &[RaftIndexedLogEntry],
    b: &[RaftIndexedLogEntry],
    applied_a: usize,
    applied_b: usize,
    pos: usize,
) -> (result: bool)
    ensures result == raft_state_machine_safety_at_spec(
        a@,
        b@,
        applied_a as int,
        applied_b as int,
        pos as int,
    )
{
    pos < applied_a && pos < applied_b && pos < a.len() && pos < b.len() && a[pos].command == b[pos].command
}

// r[verify domain-primitives.raft-consensus.state-machine-safety]
/// Admitted state-machine safety gives equal command identifiers at the shared applied position.
pub proof fn admitted_state_machine_safety_commands_equal(
    a: Seq<RaftIndexedLogEntry>,
    b: Seq<RaftIndexedLogEntry>,
    applied_a: int,
    applied_b: int,
    pos: int,
)
    requires raft_state_machine_safety_at_spec(a, b, applied_a, applied_b, pos)
    ensures a[pos].command == b[pos].command
{}

// r[depends domain-primitives.raft-consensus.term]
/// Spec: processing an observed term never decreases the current term.
pub open spec fn observed_term_update_spec(current_term: u64, observed_term: u64) -> u64 {
    if observed_term > current_term { observed_term } else { current_term }
}

// r[depends domain-primitives.raft-consensus.message]
/// Spec: a term-bearing message is stale exactly when its term is below the receiver term.
pub open spec fn stale_term_message_spec(current_term: u64, observed_term: u64) -> bool {
    observed_term < current_term
}

// r[depends domain-primitives.raft-consensus.message]
/// Spec: finite Raft message envelope kind is one of the modeled input classes.
pub open spec fn raft_message_kind_valid_spec(kind: u64) -> bool {
    RAFT_MESSAGE_REQUEST_VOTE <= kind && kind <= RAFT_MESSAGE_CLIENT_PROPOSAL
}

// r[depends domain-primitives.raft-consensus.message]
/// Spec: finite Raft message envelopes are bounded to a modeled one-based node universe.
pub open spec fn raft_message_envelope_valid_spec(message: RaftMessageEnvelope, max_node_id: u64) -> bool {
    max_node_id > 0
        && raft_message_kind_valid_spec(message.kind)
        && 1 <= message.source && message.source <= max_node_id
        && 1 <= message.target && message.target <= max_node_id
}

// r[depends domain-primitives.raft-consensus.message]
/// Spec: message admission rejects stale finite envelopes before any unsafe mutation.
pub open spec fn raft_message_envelope_admit_spec(
    current_term: u64,
    message: RaftMessageEnvelope,
    max_node_id: u64,
) -> bool {
    raft_message_envelope_valid_spec(message, max_node_id)
        && !stale_term_message_spec(current_term, message.term)
}

// r[depends domain-primitives.raft-consensus.trace]
/// Spec: a finite trace step is either an admitted term transition or a stale rejection with no term mutation.
pub open spec fn raft_transition_trace_step_admit_spec(
    step: RaftTransitionTraceStep,
    max_node_id: u64,
) -> bool {
    raft_message_envelope_valid_spec(step.message, max_node_id)
        && if stale_term_message_spec(step.before_term, step.message.term) {
            !step.admitted && step.after_term == step.before_term
        } else {
            step.admitted && step.after_term == observed_term_update_spec(step.before_term, step.message.term)
        }
}

// r[impl domain-primitives.raft-consensus.message]
/// Return whether a finite message kind is one of the modeled Raft input classes.
pub fn raft_message_kind_valid(kind: u64) -> (result: bool)
    ensures result == raft_message_kind_valid_spec(kind)
{
    RAFT_MESSAGE_REQUEST_VOTE <= kind && kind <= RAFT_MESSAGE_CLIENT_PROPOSAL
}

// r[impl domain-primitives.raft-consensus.message]
/// Return whether a finite message envelope is bounded to the modeled node universe.
pub fn raft_message_envelope_valid(message: &RaftMessageEnvelope, max_node_id: u64) -> (result: bool)
    ensures result == raft_message_envelope_valid_spec(*message, max_node_id)
{
    max_node_id > 0
        && raft_message_kind_valid(message.kind)
        && 1 <= message.source && message.source <= max_node_id
        && 1 <= message.target && message.target <= max_node_id
}

// r[impl domain-primitives.raft-consensus.message]
/// Return whether a finite message envelope is admitted for the receiver term.
pub fn raft_message_envelope_admit(
    current_term: u64,
    message: &RaftMessageEnvelope,
    max_node_id: u64,
) -> (result: bool)
    ensures result == raft_message_envelope_admit_spec(current_term, *message, max_node_id)
{
    raft_message_envelope_valid(message, max_node_id)
        && !stale_term_message(current_term, message.term)
}

// r[impl domain-primitives.raft-consensus.trace]
/// Return whether a finite trace step is an admitted transition or a safe stale rejection.
pub fn raft_transition_trace_step_admit(
    step: &RaftTransitionTraceStep,
    max_node_id: u64,
) -> (result: bool)
    ensures result == raft_transition_trace_step_admit_spec(*step, max_node_id)
{
    if !raft_message_envelope_valid(&step.message, max_node_id) {
        return false;
    }
    if stale_term_message(step.before_term, step.message.term) {
        !step.admitted && step.after_term == step.before_term
    } else {
        step.admitted && step.after_term == observed_term_update(step.before_term, step.message.term)
    }
}

// r[verify domain-primitives.raft-consensus.message]
/// Admitted finite message envelopes are valid and not stale for the receiver term.
pub proof fn admitted_message_envelope_is_valid_and_fresh(
    current_term: u64,
    message: RaftMessageEnvelope,
    max_node_id: u64,
)
    requires raft_message_envelope_admit_spec(current_term, message, max_node_id)
    ensures
        raft_message_envelope_valid_spec(message, max_node_id),
        current_term <= message.term,
{}

// r[verify domain-primitives.raft-consensus.trace]
/// Stale rejected trace steps leave the receiver term unchanged.
pub proof fn stale_rejected_trace_step_has_no_term_mutation(
    step: RaftTransitionTraceStep,
    max_node_id: u64,
)
    requires
        raft_transition_trace_step_admit_spec(step, max_node_id),
        stale_term_message_spec(step.before_term, step.message.term),
    ensures
        !step.admitted,
        step.after_term == step.before_term,
{}

// r[verify domain-primitives.raft-consensus.trace]
/// Admitted non-stale trace steps update the receiver term by the monotone observed-term rule.
pub proof fn admitted_trace_step_follows_observed_term_update(
    step: RaftTransitionTraceStep,
    max_node_id: u64,
)
    requires
        raft_transition_trace_step_admit_spec(step, max_node_id),
        step.admitted,
    ensures
        !stale_term_message_spec(step.before_term, step.message.term),
        step.after_term == observed_term_update_spec(step.before_term, step.message.term),
        step.after_term >= step.before_term,
{}

// r[depends domain-primitives.raft-consensus.invariant-bundle]
/// Spec: finite Raft invariant bundle composes local model validity predicates.
pub open spec fn raft_invariant_bundle_valid_spec(
    bundle: RaftInvariantBundleMetadata,
    log: Seq<RaftIndexedLogEntry>,
) -> bool {
    bundle.max_node_id > 0
        && bundle.current_term == bundle.persistent.current_term
        && raft_persistent_metadata_valid_spec(bundle.persistent, bundle.max_node_id)
        && raft_volatile_metadata_valid_spec(bundle.volatile)
        && log.len() <= u64::MAX as int
        && bundle.volatile.last_log_index == log.len() as u64
        && raft_log_no_gap_spec(log)
        && raft_snapshot_metadata_valid_spec(bundle.snapshot)
        && raft_membership_config_valid_spec(bundle.membership)
        && bundle.membership.max_node_id <= bundle.max_node_id
}

// r[depends domain-primitives.raft-consensus.invariant-bundle]
/// Spec: an admitted invariant-preservation candidate has valid before and after bundles.
pub open spec fn raft_invariant_preservation_admit_spec(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
) -> bool {
    raft_invariant_bundle_valid_spec(before, before_log)
        && raft_invariant_bundle_valid_spec(after, after_log)
}

// r[depends domain-primitives.raft-consensus.admission]
/// Spec: stable admission result codes are drawn from the finite Raft admission-code set.
pub open spec fn raft_admission_code_valid_spec(code: u64) -> bool {
    code <= RAFT_ADMISSION_AFTER_INVARIANT_FAILED
}

// r[depends domain-primitives.raft-consensus.admission]
/// Spec: deterministic Raft admission checks finite invariants, envelope, trace, and post-invariant in order.
pub open spec fn raft_transition_admission_result_spec(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
) -> RaftAdmissionResult {
    if !raft_invariant_bundle_valid_spec(before, before_log) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_BEFORE_INVARIANT_FAILED }
    } else if !raft_message_envelope_admit_spec(before.current_term, step.message, before.max_node_id) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_MESSAGE_FAILED }
    } else if !raft_transition_trace_step_admit_spec(step, before.max_node_id) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_TRACE_FAILED }
    } else if !raft_invariant_bundle_valid_spec(after, after_log) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_AFTER_INVARIANT_FAILED }
    } else {
        RaftAdmissionResult { admitted: true, failing_check: RAFT_ADMISSION_OK }
    }
}

// r[depends domain-primitives.raft-consensus.admission]
/// Spec: deterministic admission is true exactly for the stable OK result.
pub open spec fn raft_transition_admitted_spec(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
) -> bool {
    raft_transition_admission_result_spec(before, after, before_log, after_log, step).admitted
}

// r[impl domain-primitives.raft-consensus.invariant-bundle]
/// Return whether the finite Raft invariant bundle holds for local metadata and log.
pub fn raft_invariant_bundle_valid(
    bundle: &RaftInvariantBundleMetadata,
    log: &[RaftIndexedLogEntry],
) -> (result: bool)
    requires log@.len() <= u64::MAX as int
    ensures result == raft_invariant_bundle_valid_spec(*bundle, log@)
{
    bundle.max_node_id > 0
        && bundle.current_term == bundle.persistent.current_term
        && raft_persistent_metadata_valid(&bundle.persistent, bundle.max_node_id)
        && raft_volatile_metadata_valid(&bundle.volatile)
        && bundle.volatile.last_log_index == log.len() as u64
        && raft_log_no_gap(log)
        && raft_snapshot_metadata_valid(&bundle.snapshot)
        && raft_membership_config_valid(&bundle.membership)
        && bundle.membership.max_node_id <= bundle.max_node_id
}

// r[impl domain-primitives.raft-consensus.invariant-bundle]
/// Return whether a finite transition candidate preserves the invariant bundle.
pub fn raft_invariant_preservation_admit(
    before: &RaftInvariantBundleMetadata,
    after: &RaftInvariantBundleMetadata,
    before_log: &[RaftIndexedLogEntry],
    after_log: &[RaftIndexedLogEntry],
) -> (result: bool)
    requires
        before_log@.len() <= u64::MAX as int,
        after_log@.len() <= u64::MAX as int,
    ensures result == raft_invariant_preservation_admit_spec(*before, *after, before_log@, after_log@)
{
    raft_invariant_bundle_valid(before, before_log)
        && raft_invariant_bundle_valid(after, after_log)
}

// r[impl domain-primitives.raft-consensus.admission]
/// Return whether a deterministic Raft admission code is in the modeled finite code set.
pub fn raft_admission_code_valid(code: u64) -> (result: bool)
    ensures result == raft_admission_code_valid_spec(code)
{
    code <= RAFT_ADMISSION_AFTER_INVARIANT_FAILED
}

// r[impl domain-primitives.raft-consensus.admission]
/// Return deterministic admission metadata for a finite Raft transition candidate.
pub fn raft_transition_admission_result(
    before: &RaftInvariantBundleMetadata,
    after: &RaftInvariantBundleMetadata,
    before_log: &[RaftIndexedLogEntry],
    after_log: &[RaftIndexedLogEntry],
    step: &RaftTransitionTraceStep,
) -> (result: RaftAdmissionResult)
    requires
        before_log@.len() <= u64::MAX as int,
        after_log@.len() <= u64::MAX as int,
    ensures
        result == raft_transition_admission_result_spec(*before, *after, before_log@, after_log@, *step),
        raft_admission_code_valid_spec(result.failing_check),
        result.admitted ==> result.failing_check == RAFT_ADMISSION_OK,
        !result.admitted ==> result.failing_check != RAFT_ADMISSION_OK,
{
    if !raft_invariant_bundle_valid(before, before_log) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_BEFORE_INVARIANT_FAILED }
    } else if !raft_message_envelope_admit(before.current_term, &step.message, before.max_node_id) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_MESSAGE_FAILED }
    } else if !raft_transition_trace_step_admit(step, before.max_node_id) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_TRACE_FAILED }
    } else if !raft_invariant_bundle_valid(after, after_log) {
        RaftAdmissionResult { admitted: false, failing_check: RAFT_ADMISSION_AFTER_INVARIANT_FAILED }
    } else {
        RaftAdmissionResult { admitted: true, failing_check: RAFT_ADMISSION_OK }
    }
}

// r[impl domain-primitives.raft-consensus.admission]
/// Return whether a finite transition candidate is admitted by the deterministic API.
pub fn raft_transition_admitted(
    before: &RaftInvariantBundleMetadata,
    after: &RaftInvariantBundleMetadata,
    before_log: &[RaftIndexedLogEntry],
    after_log: &[RaftIndexedLogEntry],
    step: &RaftTransitionTraceStep,
) -> (result: bool)
    requires
        before_log@.len() <= u64::MAX as int,
        after_log@.len() <= u64::MAX as int,
    ensures result == raft_transition_admitted_spec(*before, *after, before_log@, after_log@, *step)
{
    raft_transition_admission_result(before, after, before_log, after_log, step).admitted
}

// r[verify domain-primitives.raft-consensus.admission]
/// Deterministic Raft admission results always carry a finite stable admission code.
pub proof fn transition_admission_result_code_is_valid(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
)
    ensures raft_admission_code_valid_spec(
        raft_transition_admission_result_spec(before, after, before_log, after_log, step).failing_check,
    )
{}

// r[verify domain-primitives.raft-consensus.admission]
/// Admitted deterministic transitions expose all modeled finite admission checks.
pub proof fn admitted_transition_result_exposes_checks(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
)
    requires raft_transition_admission_result_spec(before, after, before_log, after_log, step).admitted
    ensures
        raft_transition_admission_result_spec(before, after, before_log, after_log, step).failing_check == RAFT_ADMISSION_OK,
        raft_invariant_bundle_valid_spec(before, before_log),
        raft_message_envelope_admit_spec(before.current_term, step.message, before.max_node_id),
        raft_transition_trace_step_admit_spec(step, before.max_node_id),
        raft_invariant_bundle_valid_spec(after, after_log),
{}

// r[verify domain-primitives.raft-consensus.admission]
/// A failed pre-invariant check returns the stable pre-invariant rejection code.
pub proof fn admission_rejects_invalid_before_bundle(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
)
    requires !raft_invariant_bundle_valid_spec(before, before_log)
    ensures
        !raft_transition_admission_result_spec(before, after, before_log, after_log, step).admitted,
        raft_transition_admission_result_spec(before, after, before_log, after_log, step).failing_check
            == RAFT_ADMISSION_BEFORE_INVARIANT_FAILED,
{}

// r[verify domain-primitives.raft-consensus.admission]
/// Failed message admission returns the stable message rejection code after a valid pre-bundle.
pub proof fn admission_rejects_invalid_message(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
)
    requires
        raft_invariant_bundle_valid_spec(before, before_log),
        !raft_message_envelope_admit_spec(before.current_term, step.message, before.max_node_id),
    ensures
        !raft_transition_admission_result_spec(before, after, before_log, after_log, step).admitted,
        raft_transition_admission_result_spec(before, after, before_log, after_log, step).failing_check
            == RAFT_ADMISSION_MESSAGE_FAILED,
{}

// r[verify domain-primitives.raft-consensus.admission]
/// Failed trace admission returns the stable trace rejection code after prechecks pass.
pub proof fn admission_rejects_invalid_trace(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
)
    requires
        raft_invariant_bundle_valid_spec(before, before_log),
        raft_message_envelope_admit_spec(before.current_term, step.message, before.max_node_id),
        !raft_transition_trace_step_admit_spec(step, before.max_node_id),
    ensures
        !raft_transition_admission_result_spec(before, after, before_log, after_log, step).admitted,
        raft_transition_admission_result_spec(before, after, before_log, after_log, step).failing_check
            == RAFT_ADMISSION_TRACE_FAILED,
{}

// r[verify domain-primitives.raft-consensus.admission]
/// Failed post-invariant admission returns the stable post-invariant rejection code after prechecks pass.
pub proof fn admission_rejects_invalid_after_bundle(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
)
    requires
        raft_invariant_bundle_valid_spec(before, before_log),
        raft_message_envelope_admit_spec(before.current_term, step.message, before.max_node_id),
        raft_transition_trace_step_admit_spec(step, before.max_node_id),
        !raft_invariant_bundle_valid_spec(after, after_log),
    ensures
        !raft_transition_admission_result_spec(before, after, before_log, after_log, step).admitted,
        raft_transition_admission_result_spec(before, after, before_log, after_log, step).failing_check
            == RAFT_ADMISSION_AFTER_INVARIANT_FAILED,
{}

// r[verify domain-primitives.raft-consensus.invariant-bundle]
/// Admitted invariant-preservation candidates expose a valid post-transition bundle.
pub proof fn admitted_invariant_preservation_has_after_bundle(
    before: RaftInvariantBundleMetadata,
    after: RaftInvariantBundleMetadata,
    before_log: Seq<RaftIndexedLogEntry>,
    after_log: Seq<RaftIndexedLogEntry>,
)
    requires raft_invariant_preservation_admit_spec(before, after, before_log, after_log)
    ensures raft_invariant_bundle_valid_spec(after, after_log)
{}

// r[verify domain-primitives.raft-consensus.invariant-bundle]
/// No-mutation admissions such as rejected stale inputs or read-only checks preserve the bundle unchanged.
pub proof fn no_mutation_transition_preserves_invariant_bundle(
    before: RaftInvariantBundleMetadata,
    log: Seq<RaftIndexedLogEntry>,
)
    requires raft_invariant_bundle_valid_spec(before, log)
    ensures raft_invariant_bundle_valid_spec(before, log)
{}

// r[verify domain-primitives.raft-consensus.invariant-bundle]
/// Observed-term transitions preserve the bundle when persisted metadata follows the same rule.
pub proof fn observed_term_transition_preserves_invariant_bundle(
    before: RaftInvariantBundleMetadata,
    log: Seq<RaftIndexedLogEntry>,
    observed_term: u64,
)
    requires raft_invariant_bundle_valid_spec(before, log)
    ensures raft_invariant_bundle_valid_spec(
        RaftInvariantBundleMetadata {
            current_term: observed_term_update_spec(before.current_term, observed_term),
            persistent: raft_persistent_observe_term_spec(before.persistent, observed_term),
            volatile: before.volatile,
            snapshot: before.snapshot,
            membership: before.membership,
            max_node_id: before.max_node_id,
        },
        log,
    )
{
    if observed_term > before.persistent.current_term {
        assert(raft_persistent_observe_term_spec(before.persistent, observed_term).voted_for == 0);
        assert(raft_persistent_metadata_valid_spec(raft_persistent_observe_term_spec(before.persistent, observed_term), before.max_node_id));
    } else {
        assert(raft_persistent_observe_term_spec(before.persistent, observed_term) == before.persistent);
    }
}

// r[verify domain-primitives.raft-consensus.invariant-bundle]
/// Commit-index advancement preserves the bundle when the volatile commit rule admits the new index.
pub proof fn commit_advance_preserves_invariant_bundle(
    before: RaftInvariantBundleMetadata,
    log: Seq<RaftIndexedLogEntry>,
    new_commit: u64,
)
    requires
        raft_invariant_bundle_valid_spec(before, log),
        raft_commit_advance_admit_spec(before.volatile, new_commit),
    ensures raft_invariant_bundle_valid_spec(
        RaftInvariantBundleMetadata {
            current_term: before.current_term,
            persistent: before.persistent,
            volatile: RaftVolatileMetadata {
                commit_index: new_commit,
                last_applied: before.volatile.last_applied,
                last_log_index: before.volatile.last_log_index,
            },
            snapshot: before.snapshot,
            membership: before.membership,
            max_node_id: before.max_node_id,
        },
        log,
    )
{
    admitted_commit_advance_is_ordered(before.volatile, new_commit);
}

// r[verify domain-primitives.raft-consensus.invariant-bundle]
/// Membership epoch changes preserve the bundle when new membership remains inside the modeled universe.
pub proof fn membership_epoch_transition_preserves_invariant_bundle(
    before: RaftInvariantBundleMetadata,
    log: Seq<RaftIndexedLogEntry>,
    new_membership: RaftMembershipConfig,
)
    requires
        raft_invariant_bundle_valid_spec(before, log),
        raft_membership_epoch_advance_admit_spec(before.membership, new_membership),
        new_membership.max_node_id <= before.max_node_id,
    ensures raft_invariant_bundle_valid_spec(
        RaftInvariantBundleMetadata {
            current_term: before.current_term,
            persistent: before.persistent,
            volatile: before.volatile,
            snapshot: before.snapshot,
            membership: new_membership,
            max_node_id: before.max_node_id,
        },
        log,
    )
{}

// r[verify domain-primitives.raft-consensus.invariant-bundle]
/// InstallSnapshot transitions preserve the bundle for admitted prefix-compatible snapshot metadata.
pub proof fn install_snapshot_transition_preserves_invariant_bundle(
    before: RaftInvariantBundleMetadata,
    log: Seq<RaftIndexedLogEntry>,
    leader_term: u64,
    snapshot: RaftSnapshotMetadata,
)
    requires
        raft_invariant_bundle_valid_spec(before, log),
        raft_install_snapshot_admit_spec(before.current_term, leader_term, before.snapshot.last_included_index, log, snapshot),
    ensures raft_invariant_bundle_valid_spec(
        RaftInvariantBundleMetadata {
            current_term: observed_term_update_spec(before.current_term, leader_term),
            persistent: raft_persistent_observe_term_spec(before.persistent, leader_term),
            volatile: before.volatile,
            snapshot: snapshot,
            membership: before.membership,
            max_node_id: before.max_node_id,
        },
        log,
    )
{
    admitted_install_snapshot_is_newer(before.current_term, leader_term, before.snapshot.last_included_index, log, snapshot);
    observed_term_transition_preserves_invariant_bundle(before, log, leader_term);
}

// r[verify domain-primitives.raft-consensus.invariant-bundle]
/// Message trace transitions preserve the bundle when the term update mirrors the admitted trace step.
pub proof fn transition_trace_step_preserves_invariant_bundle(
    before: RaftInvariantBundleMetadata,
    log: Seq<RaftIndexedLogEntry>,
    step: RaftTransitionTraceStep,
)
    requires
        raft_invariant_bundle_valid_spec(before, log),
        step.before_term == before.current_term,
        raft_transition_trace_step_admit_spec(step, before.max_node_id),
    ensures raft_invariant_bundle_valid_spec(
        RaftInvariantBundleMetadata {
            current_term: step.after_term,
            persistent: raft_persistent_observe_term_spec(before.persistent, step.message.term),
            volatile: before.volatile,
            snapshot: before.snapshot,
            membership: before.membership,
            max_node_id: before.max_node_id,
        },
        log,
    )
{
    if stale_term_message_spec(step.before_term, step.message.term) {
        stale_rejected_trace_step_has_no_term_mutation(step, before.max_node_id);
        assert(step.message.term < before.persistent.current_term);
        assert(raft_persistent_observe_term_spec(before.persistent, step.message.term) == before.persistent);
    } else {
        assert(step.after_term == observed_term_update_spec(before.current_term, step.message.term));
        observed_term_transition_preserves_invariant_bundle(before, log, step.message.term);
    }
}

// r[impl domain-primitives.raft-consensus.term]
/// Return the successor current term after observing a term-bearing Raft input.
pub fn observed_term_update(current_term: u64, observed_term: u64) -> (result: u64)
    ensures
        result == observed_term_update_spec(current_term, observed_term),
        result >= current_term,
        result >= observed_term || observed_term < current_term,
{
    if observed_term > current_term { observed_term } else { current_term }
}

// r[impl domain-primitives.raft-consensus.message]
/// Return whether a term-bearing Raft input is stale for the receiver.
pub fn stale_term_message(current_term: u64, observed_term: u64) -> (result: bool)
    ensures result == stale_term_message_spec(current_term, observed_term)
{
    observed_term < current_term
}

// r[verify domain-primitives.raft-consensus.term]
/// Observing any term is a monotone local-term transition.
pub proof fn observed_term_update_monotone(current_term: u64, observed_term: u64)
    ensures
        observed_term_update_spec(current_term, observed_term) >= current_term,
        observed_term_update_spec(current_term, observed_term) == current_term
            || observed_term_update_spec(current_term, observed_term) == observed_term,
{}

// r[verify domain-primitives.raft-consensus.message]
/// Lower observed terms are rejected as stale and cannot decrease the receiver term.
pub proof fn stale_observed_term_keeps_current(current_term: u64, observed_term: u64)
    requires observed_term < current_term
    ensures
        stale_term_message_spec(current_term, observed_term),
        observed_term_update_spec(current_term, observed_term) == current_term,
{}

// r[depends domain-primitives.raft-consensus.log-entry]
/// Spec: an indexed Raft log entry is inside the declared finite bounds.
pub open spec fn raft_log_entry_in_bounds_spec(
    entry: RaftIndexedLogEntry,
    max_index: u64,
    max_term: u64,
) -> bool {
    1 <= entry.index && entry.index <= max_index && entry.term <= max_term
}

// r[depends domain-primitives.raft-consensus.log-prefix]
/// Spec: an indexed Raft log has no gaps exactly when entry indexes are 1-based and contiguous.
pub open spec fn raft_log_no_gap_spec(log: Seq<RaftIndexedLogEntry>) -> bool {
    log.len() <= u64::MAX as int
        && forall|i: int| #![trigger log[i]]
            0 <= i < log.len() ==> log[i].index == (i + 1) as u64
}

// r[depends domain-primitives.raft-consensus.log-prefix]
/// Spec: `prefix` is an exact entry prefix of `log`.
pub open spec fn raft_log_prefix_spec(
    prefix: Seq<RaftIndexedLogEntry>,
    log: Seq<RaftIndexedLogEntry>,
) -> bool {
    prefix.len() <= log.len()
        && forall|i: int| #![trigger log[i]]
            0 <= i < prefix.len() ==> log[i] == prefix[i]
}

// r[depends domain-primitives.raft-consensus.log-prefix]
/// Spec: appending a single entry to the finite indexed log.
pub open spec fn raft_log_append_spec(
    log: Seq<RaftIndexedLogEntry>,
    entry: RaftIndexedLogEntry,
) -> Seq<RaftIndexedLogEntry> {
    log.push(entry)
}

// r[impl domain-primitives.raft-consensus.log-entry]
/// Return whether an explicit Raft log-entry envelope is inside finite bounds.
pub fn raft_log_entry_in_bounds(
    entry: &RaftIndexedLogEntry,
    max_index: u64,
    max_term: u64,
) -> (result: bool)
    ensures result == raft_log_entry_in_bounds_spec(*entry, max_index, max_term)
{
    1 <= entry.index && entry.index <= max_index && entry.term <= max_term
}

// r[impl domain-primitives.raft-consensus.log-prefix]
/// Return whether indexed entries form the contiguous one-based Raft log range `1..=len`.
pub fn raft_log_no_gap(log: &[RaftIndexedLogEntry]) -> (result: bool)
    requires log@.len() <= u64::MAX as int
    ensures result == raft_log_no_gap_spec(log@)
{
    let mut i: usize = 0;
    while i < log.len()
        invariant
            0 <= i <= log@.len(),
            log@.len() <= u64::MAX as int,
            forall|j: int| #![trigger log@[j]]
                0 <= j < i as int ==> log@[j].index == (j + 1) as u64,
        decreases log@.len() - i
    {
        if log[i].index != i as u64 + 1 {
            assert(0 <= i as int);
            assert((i as int) < log@.len());
            assert(log@[i as int].index != (i as int + 1) as u64);
            assert(!raft_log_no_gap_spec(log@));
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl domain-primitives.raft-consensus.log-prefix]
/// Return whether `prefix` is an exact entry prefix of `log`.
pub fn raft_log_prefix(
    prefix: &[RaftIndexedLogEntry],
    log: &[RaftIndexedLogEntry],
) -> (result: bool)
    ensures result == raft_log_prefix_spec(prefix@, log@)
{
    if prefix.len() > log.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < prefix.len()
        invariant
            0 <= i <= prefix@.len(),
            prefix@.len() <= log@.len(),
            forall|j: int| #![trigger log@[j]]
                0 <= j < i as int ==> log@[j] == prefix@[j],
        decreases prefix@.len() - i
    {
        if log[i].index != prefix[i].index
            || log[i].term != prefix[i].term
            || log[i].command != prefix[i].command
        {
            assert(0 <= i as int);
            assert((i as int) < prefix@.len());
            assert(log@[i as int] != prefix@[i as int]);
            assert(!raft_log_prefix_spec(prefix@, log@));
            return false;
        }
        i = i + 1;
    }
    true
}

// r[impl domain-primitives.raft-consensus.log-prefix]
/// Append a contiguous entry to an explicit indexed Raft log.
pub fn raft_log_append(
    log: &mut Vec<RaftIndexedLogEntry>,
    entry: RaftIndexedLogEntry,
)
    requires
        old(log)@.len() < u64::MAX as int,
        entry.index == old(log)@.len() as u64 + 1,
    ensures
        log@ == raft_log_append_spec(old(log)@, entry),
        log@.len() == old(log)@.len() + 1,
        log@[old(log)@.len() as int] == entry,
        forall|i: int| 0 <= i < old(log)@.len() ==> log@[i] == old(log)@[i],
        raft_log_no_gap_spec(old(log)@) ==> raft_log_no_gap_spec(log@),
{
    log.push(entry);
}

// r[verify domain-primitives.raft-consensus.log-entry]
/// A valid entry always has a nonzero one-based index.
pub proof fn admitted_log_entry_has_nonzero_index(
    entry: RaftIndexedLogEntry,
    max_index: u64,
    max_term: u64,
)
    requires raft_log_entry_in_bounds_spec(entry, max_index, max_term)
    ensures entry.index > 0
{}

// r[verify domain-primitives.raft-consensus.log-prefix]
/// Empty indexed logs are gap-free.
pub proof fn empty_raft_log_has_no_gaps()
    ensures raft_log_no_gap_spec(Seq::<RaftIndexedLogEntry>::empty())
{}

// r[verify domain-primitives.raft-consensus.log-prefix]
/// Contiguous append preserves the no-gap predicate.
pub proof fn contiguous_append_preserves_no_gap(log: Seq<RaftIndexedLogEntry>, entry: RaftIndexedLogEntry)
    requires
        log.len() < u64::MAX as int,
        raft_log_no_gap_spec(log),
        entry.index == log.len() as u64 + 1,
    ensures raft_log_no_gap_spec(raft_log_append_spec(log, entry))
{}

// r[verify domain-primitives.raft-consensus.log-prefix]
/// A prefix of a no-gap log is also no-gap.
pub proof fn prefix_of_no_gap_is_no_gap(
    prefix: Seq<RaftIndexedLogEntry>,
    log: Seq<RaftIndexedLogEntry>,
)
    requires
        raft_log_no_gap_spec(log),
        raft_log_prefix_spec(prefix, log),
    ensures raft_log_no_gap_spec(prefix)
{
    assert(prefix.len() <= log.len());
    assert(log.len() <= u64::MAX as int);
    assert(prefix.len() <= u64::MAX as int);
    assert forall|i: int| #![trigger prefix[i]]
        0 <= i < prefix.len() implies prefix[i].index == (i + 1) as u64 by {
            assert(log[i] == prefix[i]);
            assert(log[i].index == (i + 1) as u64);
        }
}

// r[depends domain-primitives.raft-consensus.append-entries]
/// Spec: this finite AppendEntries admission slice combines non-stale term and prev-log check.
pub open spec fn raft_append_entries_admit_spec(
    current_term: u64,
    leader_term: u64,
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
) -> bool {
    !stale_term_message_spec(current_term, leader_term)
        && check_append_entries_spec(follower_log, prev_log_index, prev_log_term)
}

// r[impl domain-primitives.raft-consensus.append-entries]
/// Return whether this finite AppendEntries request is admitted by term and prev-log checks.
pub fn raft_append_entries_admit(
    current_term: u64,
    leader_term: u64,
    follower_log: &[LogEntry],
    prev_log_index: u64,
    prev_log_term: u64,
) -> (result: bool)
    requires prev_log_index as int <= usize::MAX as int
    ensures result == raft_append_entries_admit_spec(
        current_term,
        leader_term,
        follower_log@,
        prev_log_index,
        prev_log_term,
    )
{
    if stale_term_message(current_term, leader_term) {
        false
    } else {
        check_append_entries(follower_log, prev_log_index, prev_log_term)
    }
}

// r[verify domain-primitives.raft-consensus.append-entries]
/// Stale AppendEntries requests are rejected before any prev-log admission can accept them.
pub proof fn stale_append_entries_rejected(
    current_term: u64,
    leader_term: u64,
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires leader_term < current_term
    ensures !raft_append_entries_admit_spec(
        current_term,
        leader_term,
        follower_log,
        prev_log_index,
        prev_log_term,
    )
{}

// r[verify domain-primitives.raft-consensus.append-entries]
/// Previous-log mismatch rejects AppendEntries in the non-stale term case.
pub proof fn append_entries_prev_log_mismatch_rejected(
    current_term: u64,
    leader_term: u64,
    follower_log: Seq<LogEntry>,
    prev_log_index: u64,
    prev_log_term: u64,
)
    requires
        leader_term >= current_term,
        !check_append_entries_spec(follower_log, prev_log_index, prev_log_term),
    ensures !raft_append_entries_admit_spec(
        current_term,
        leader_term,
        follower_log,
        prev_log_index,
        prev_log_term,
    )
{}

// r[depends domain-primitives.raft-consensus.vote]
/// Spec: finite Raft vote admission delegates to the verified ballot predicate.
pub open spec fn raft_vote_grant_spec(
    state: VoteState,
    candidate: u64,
    candidate_term: u64,
    candidate_last: Epoch,
    voter_last: Epoch,
) -> bool {
    can_grant_vote_spec(state, candidate, candidate_term, candidate_last, voter_last)
}

// r[depends domain-primitives.raft-consensus.request-vote]
/// Spec: RequestVote admission for this slice is the vote-grant predicate plus non-stale term.
pub open spec fn request_vote_admit_spec(
    state: VoteState,
    candidate: u64,
    candidate_term: u64,
    candidate_last: Epoch,
    voter_last: Epoch,
) -> bool {
    !stale_term_message_spec(state.voted_term, candidate_term)
        && raft_vote_grant_spec(state, candidate, candidate_term, candidate_last, voter_last)
}

// r[impl domain-primitives.raft-consensus.vote]
/// Return whether the finite vote-grant predicate admits this candidate.
pub fn raft_vote_grant(
    state: &VoteState,
    candidate: u64,
    candidate_term: u64,
    candidate_last: &Epoch,
    voter_last: &Epoch,
) -> (result: bool)
    ensures result == raft_vote_grant_spec(*state, candidate, candidate_term, *candidate_last, *voter_last)
{
    can_grant_vote(state, candidate, candidate_term, candidate_last, voter_last)
}

// r[impl domain-primitives.raft-consensus.request-vote]
/// Return whether a finite RequestVote input is admitted by term and log freshness checks.
pub fn request_vote_admit(
    state: &VoteState,
    candidate: u64,
    candidate_term: u64,
    candidate_last: &Epoch,
    voter_last: &Epoch,
) -> (result: bool)
    ensures result == request_vote_admit_spec(*state, candidate, candidate_term, *candidate_last, *voter_last)
{
    if stale_term_message(state.voted_term, candidate_term) {
        false
    } else {
        raft_vote_grant(state, candidate, candidate_term, candidate_last, voter_last)
    }
}

// r[verify domain-primitives.raft-consensus.vote]
// r[verify domain-primitives.raft-consensus.request-vote]
/// Once a voter records a vote for one candidate, a distinct candidate is rejected in the same term.
pub proof fn second_vote_same_term_rejected(
    candidate_a: u64,
    candidate_b: u64,
    term: u64,
    candidate_b_last: Epoch,
    voter_last: Epoch,
)
    requires candidate_a != candidate_b
    ensures !request_vote_admit_spec(
        grant_vote_spec(candidate_a, term),
        candidate_b,
        term,
        candidate_b_last,
        voter_last,
    )
{}

// r[depends domain-primitives.raft-consensus.election-safety]
/// Spec: two admitted leaders for the same term are compatible only if they are the same node.
pub open spec fn same_term_leaders_compatible_spec(candidate_a: u64, candidate_b: u64) -> bool {
    candidate_a == candidate_b
}

// r[impl domain-primitives.raft-consensus.election-safety]
/// Return whether two same-term leader witnesses are compatible with election safety.
pub fn same_term_leaders_compatible(candidate_a: u64, candidate_b: u64) -> (result: bool)
    ensures result == same_term_leaders_compatible_spec(candidate_a, candidate_b)
{
    candidate_a == candidate_b
}

// r[verify domain-primitives.raft-consensus.election-safety]
/// Distinct same-term leader witnesses violate election safety.
pub proof fn distinct_same_term_leaders_incompatible(candidate_a: u64, candidate_b: u64)
    requires candidate_a != candidate_b
    ensures !same_term_leaders_compatible_spec(candidate_a, candidate_b)
{}

// r[depends domain-primitives.raft-consensus.election-safety]
/// Spec: the finite vote history satisfies Raft election safety for a term.
pub open spec fn raft_election_safe_spec(
    vote_history: Seq<VoteRecord>,
    term: u64,
    cluster_size: u64,
) -> bool
    recommends cluster_size > 0
{
    election_safe_spec(vote_history, term, cluster_size)
}

// r[verify domain-primitives.raft-consensus.election-safety]
/// Single-vote-per-term histories compose with quorum intersection into election safety.
pub proof fn prove_raft_election_safety(
    vote_history: Seq<VoteRecord>,
    term: u64,
    cluster_size: u64,
)
    requires
        cluster_size > 0,
        single_vote_per_term_spec(vote_history),
    ensures raft_election_safe_spec(vote_history, term, cluster_size)
{
    election_safety_theorem(vote_history, term, cluster_size);
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ballot::VoteState;
    use crate::epoch::Epoch;
    use crate::log::LogEntry;

    fn epoch(term: u64, index: u64) -> Epoch {
        Epoch { term, index }
    }

    fn indexed(index: u64, term: u64, command: u64) -> RaftIndexedLogEntry {
        RaftIndexedLogEntry { index, term, command }
    }

    fn log_entry(term: u64, value: u64) -> LogEntry {
        LogEntry { term, value }
    }

    fn raft_model() -> RaftModelEnvelope {
        RaftModelEnvelope {
            max_node_id: 5,
            max_term: 10,
            max_log_index: 20,
            max_command_id: 100,
            max_config_epoch: 7,
        }
    }

    fn leader_state() -> RaftNodeStateMetadata {
        RaftNodeStateMetadata {
            role: RAFT_NODE_ROLE_LEADER,
            node_id: 2,
            current_term: 4,
            role_term: 4,
            voted_for: 2,
            last_log_term: 4,
            last_log_index: 6,
            commit_index: 5,
            last_applied: 3,
            config_epoch: 2,
        }
    }

    // r[verify domain-primitives.raft-consensus.model]
    #[test]
    fn raft_model_envelope_rejects_unknown_and_out_of_bound_references() {
        let model = raft_model();
        let admitted = raft_model_admission_result(&model, 3, 4, 6, 42, RAFT_MESSAGE_APPEND_ENTRIES, 2);
        assert!(admitted.admitted);
        assert_eq!(admitted.failing_check, RAFT_MODEL_OK);

        let unknown = raft_model_admission_result(&model, 0, 4, 6, 42, RAFT_MESSAGE_APPEND_ENTRIES, 2);
        assert!(!unknown.admitted);
        assert_eq!(unknown.failing_check, RAFT_MODEL_UNKNOWN_NODE);

        let bad_term = raft_model_admission_result(&model, 3, 11, 6, 42, RAFT_MESSAGE_APPEND_ENTRIES, 2);
        assert!(!bad_term.admitted);
        assert_eq!(bad_term.failing_check, RAFT_MODEL_TERM_OUT_OF_BOUNDS);

        let bad_kind = raft_model_admission_result(&model, 3, 4, 6, 42, 99, 2);
        assert!(!bad_kind.admitted);
        assert_eq!(bad_kind.failing_check, RAFT_MODEL_MESSAGE_KIND_OUT_OF_BOUNDS);
    }

    // r[verify domain-primitives.raft-consensus.node-state]
    #[test]
    fn node_state_admission_checks_role_terms_apply_bounds_and_candidate_vote() {
        let model = raft_model();
        let leader = leader_state();
        assert!(raft_node_state_admit(&model, &leader));

        let inconsistent_leader = RaftNodeStateMetadata { role_term: 3, ..leader };
        let rejected = raft_node_state_admission_result(&model, &inconsistent_leader);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_NODE_STATE_INCONSISTENT_LEADER_TERM);

        let applied_after_commit = RaftNodeStateMetadata {
            last_applied: 6,
            ..leader
        };
        let rejected = raft_node_state_admission_result(&model, &applied_after_commit);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_NODE_STATE_APPLIED_AFTER_COMMIT);

        let candidate = RaftNodeStateMetadata {
            role: RAFT_NODE_ROLE_CANDIDATE,
            node_id: 3,
            current_term: 5,
            role_term: 5,
            voted_for: 3,
            ..leader
        };
        assert!(raft_node_state_admit(&model, &candidate));

        let wrong_vote = RaftNodeStateMetadata {
            voted_for: 2,
            ..candidate
        };
        let rejected = raft_node_state_admission_result(&model, &wrong_vote);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_NODE_STATE_INCONSISTENT_CANDIDATE_VOTE);
    }

    // r[verify domain-primitives.raft-consensus.applied-prefix]
    #[test]
    fn applied_prefix_admission_requires_committed_contiguous_local_progress() {
        let step = RaftAppliedPrefixMetadata {
            old_last_applied: 2,
            first_applied: 3,
            new_last_applied: 5,
            commit_index: 5,
            last_log_index: 6,
        };
        assert!(raft_applied_prefix_admit(&step));

        let uncommitted = RaftAppliedPrefixMetadata {
            commit_index: 4,
            ..step
        };
        let rejected = raft_applied_prefix_admission_result(&uncommitted);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_APPLIED_PREFIX_UNCOMMITTED);

        let skipped = RaftAppliedPrefixMetadata {
            first_applied: 4,
            ..step
        };
        let rejected = raft_applied_prefix_admission_result(&skipped);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_APPLIED_PREFIX_SKIPPED);

        let noop = RaftAppliedPrefixMetadata {
            old_last_applied: 5,
            first_applied: 0,
            new_last_applied: 5,
            commit_index: 5,
            last_log_index: 6,
        };
        assert!(raft_applied_prefix_admit(&noop));
    }

    // r[verify domain-primitives.raft-consensus.client-proposal]
    #[test]
    fn client_proposal_admission_requires_current_leader_and_model_command() {
        let model = raft_model();
        let leader = leader_state();
        let proposal = RaftClientProposalEnvelope {
            target_node: 2,
            proposal_term: 4,
            command_id: 77,
        };
        assert!(raft_client_proposal_admit(&model, &proposal, &leader));

        let follower = RaftNodeStateMetadata {
            role: RAFT_NODE_ROLE_FOLLOWER,
            ..leader
        };
        let rejected = raft_client_proposal_admission_result(&model, &proposal, &follower);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_CLIENT_PROPOSAL_NOT_LEADER);

        let stale = RaftClientProposalEnvelope {
            proposal_term: 3,
            ..proposal
        };
        let rejected = raft_client_proposal_admission_result(&model, &stale, &leader);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_CLIENT_PROPOSAL_STALE_TERM);

        let unknown_command = RaftClientProposalEnvelope {
            command_id: 101,
            ..proposal
        };
        let rejected = raft_client_proposal_admission_result(&model, &unknown_command, &leader);
        assert!(!rejected.admitted);
        assert_eq!(rejected.failing_check, RAFT_CLIENT_PROPOSAL_MODEL_FAILED);
    }

    // r[verify domain-primitives.raft-consensus.openraft-log-pointers]
    // r[verify domain-primitives.raft-consensus.openraft-io-progress]
    // r[verify domain-primitives.raft-consensus.openraft-vote-log-io]
    // r[verify domain-primitives.raft-consensus.openraft-effective-membership]
    // r[verify domain-primitives.raft-consensus.openraft-replication-session]
    // r[verify domain-primitives.raft-consensus.openraft-read-linearizer]
    #[test]
    fn openraft_admission_metadata_reports_ok_for_valid_operational_records() {
        let ptrs = RaftOperationalLogPointers {
            purged: 1,
            snapshot: 2,
            applied: 3,
            committed: 4,
            flushed: 3,
            submitted: 5,
            accepted: 6,
        };
        let pointer_result = raft_openraft_log_pointers_admission_result(&ptrs);
        assert!(pointer_result.admitted);
        assert_eq!(pointer_result.failing_check, RAFT_OPENRAFT_OK);

        let old_progress = RaftIoProgressMetadata {
            flushed: 1,
            submitted: 2,
            accepted: 3,
        };
        let new_progress = RaftIoProgressMetadata {
            flushed: 2,
            submitted: 3,
            accepted: 4,
        };
        let io_result = raft_openraft_io_progress_admission_result(&old_progress, &new_progress);
        assert!(io_result.admitted);
        assert_eq!(io_result.failing_check, RAFT_OPENRAFT_OK);

        let vote_io = RaftVoteLogIoMetadata {
            persisted_term: 5,
            log_term: 4,
        };
        assert!(raft_openraft_vote_log_io_admission_result(&vote_io).admitted);

        let membership = RaftEffectiveMembershipMetadata {
            committed_epoch: 7,
            effective_epoch: 8,
        };
        assert!(raft_openraft_effective_membership_admission_result(&membership).admitted);

        let session = RaftReplicationSessionMetadata {
            leader_term: 9,
            leader_node: 2,
            membership_log_index: 11,
        };
        let replication_result = raft_openraft_replication_progress_admission_result(&session, &session, 3, 4, 5);
        assert!(replication_result.admitted);
        assert_eq!(replication_result.failing_check, RAFT_OPENRAFT_OK);

        let read = RaftReadLinearizerMetadata {
            read_log_index: 5,
            last_applied_index: 5,
        };
        assert!(raft_openraft_read_linearizer_admission_result(&read).admitted);
    }

    // r[verify domain-primitives.raft-consensus.openraft-log-pointers]
    // r[verify domain-primitives.raft-consensus.openraft-io-progress]
    // r[verify domain-primitives.raft-consensus.openraft-vote-log-io]
    // r[verify domain-primitives.raft-consensus.openraft-effective-membership]
    // r[verify domain-primitives.raft-consensus.openraft-replication-session]
    // r[verify domain-primitives.raft-consensus.openraft-read-linearizer]
    #[test]
    fn openraft_admission_metadata_reports_stable_first_failures() {
        let ptrs = RaftOperationalLogPointers {
            purged: 9,
            snapshot: 2,
            applied: 1,
            committed: 0,
            flushed: 8,
            submitted: 3,
            accepted: 2,
        };
        let pointer_result = raft_openraft_log_pointers_admission_result(&ptrs);
        assert!(!pointer_result.admitted);
        assert_eq!(pointer_result.failing_check, RAFT_OPENRAFT_PURGED_AFTER_SNAPSHOT);

        let invalid_old = RaftIoProgressMetadata {
            flushed: 4,
            submitted: 3,
            accepted: 5,
        };
        let regressing_new = RaftIoProgressMetadata {
            flushed: 1,
            submitted: 2,
            accepted: 1,
        };
        let io_result = raft_openraft_io_progress_admission_result(&invalid_old, &regressing_new);
        assert!(!io_result.admitted);
        assert_eq!(io_result.failing_check, RAFT_OPENRAFT_OLD_PROGRESS_INVALID);

        let vote_io = RaftVoteLogIoMetadata {
            persisted_term: 2,
            log_term: 3,
        };
        let vote_result = raft_openraft_vote_log_io_admission_result(&vote_io);
        assert!(!vote_result.admitted);
        assert_eq!(vote_result.failing_check, RAFT_OPENRAFT_PRIOR_TERM_METADATA_REQUIRED);

        let bad_membership = RaftEffectiveMembershipMetadata {
            committed_epoch: 9,
            effective_epoch: 8,
        };
        let membership_result = raft_openraft_effective_membership_admission_result(&bad_membership);
        assert!(!membership_result.admitted);
        assert_eq!(membership_result.failing_check, RAFT_OPENRAFT_EFFECTIVE_BEFORE_COMMITTED);

        let current = RaftReplicationSessionMetadata {
            leader_term: 4,
            leader_node: 2,
            membership_log_index: 10,
        };
        let stale = RaftReplicationSessionMetadata {
            leader_term: 3,
            ..current
        };
        let replication_result = raft_openraft_replication_progress_admission_result(&current, &stale, 8, 7, 6);
        assert!(!replication_result.admitted);
        assert_eq!(replication_result.failing_check, RAFT_OPENRAFT_STALE_REPLICATION_SESSION);

        let read = RaftReadLinearizerMetadata {
            read_log_index: 6,
            last_applied_index: 5,
        };
        let read_result = raft_openraft_read_linearizer_admission_result(&read);
        assert!(!read_result.admitted);
        assert_eq!(read_result.failing_check, RAFT_OPENRAFT_APPLY_CATCHUP_REQUIRED);
    }

    // r[verify domain-primitives.raft-consensus.term]
    #[test]
    fn observed_term_update_is_monotone() {
        assert_eq!(observed_term_update(5, 3), 5);
        assert_eq!(observed_term_update(5, 5), 5);
        assert_eq!(observed_term_update(5, 8), 8);
    }

    // r[verify domain-primitives.raft-consensus.message]
    #[test]
    fn stale_term_detection_matches_ordering() {
        assert!(stale_term_message(5, 4));
        assert!(!stale_term_message(5, 5));
        assert!(!stale_term_message(5, 6));
    }

    // r[verify domain-primitives.raft-consensus.persistent-state]
    #[test]
    fn persistent_metadata_observed_term_clears_vote_only_on_advance() {
        let meta = RaftPersistentMetadata {
            current_term: 3,
            voted_for: 9,
        };
        assert!(raft_persistent_metadata_valid(&meta, 10));
        assert!(!raft_persistent_metadata_valid(&meta, 8));

        let unchanged = raft_persistent_observe_term(&meta, 3);
        assert_eq!(unchanged.current_term, 3);
        assert_eq!(unchanged.voted_for, 9);

        let advanced = raft_persistent_observe_term(&meta, 4);
        assert_eq!(advanced.current_term, 4);
        assert_eq!(advanced.voted_for, 0);
    }

    // r[verify domain-primitives.raft-consensus.volatile-state]
    #[test]
    fn volatile_metadata_orders_commit_and_log_indexes() {
        let meta = RaftVolatileMetadata {
            last_applied: 2,
            commit_index: 4,
            last_log_index: 6,
        };
        assert!(raft_volatile_metadata_valid(&meta));
        assert!(raft_commit_advance_admit(&meta, 4));
        assert!(raft_commit_advance_admit(&meta, 6));
        assert!(!raft_commit_advance_admit(&meta, 3));
        assert!(!raft_commit_advance_admit(&meta, 7));

        let invalid = RaftVolatileMetadata {
            last_applied: 5,
            commit_index: 4,
            last_log_index: 6,
        };
        assert!(!raft_volatile_metadata_valid(&invalid));
    }

    // r[verify domain-primitives.raft-consensus.election-safety]
    #[test]
    fn same_term_leader_witnesses_must_match() {
        assert!(same_term_leaders_compatible(7, 7));
        assert!(!same_term_leaders_compatible(7, 8));
    }

    // r[verify domain-primitives.raft-consensus.vote]
    // r[verify domain-primitives.raft-consensus.request-vote]
    #[test]
    fn request_vote_rejects_second_candidate_same_term() {
        let voted = VoteState {
            voted_term: 7,
            voted_for: 11,
        };
        assert!(!request_vote_admit(&voted, 12, 7, &epoch(7, 10), &epoch(7, 9)));
        assert!(request_vote_admit(&voted, 11, 7, &epoch(7, 10), &epoch(7, 9)));
    }

    // r[verify domain-primitives.raft-consensus.request-vote]
    #[test]
    fn request_vote_rejects_stale_log() {
        let fresh = VoteState {
            voted_term: 0,
            voted_for: 0,
        };
        assert!(!request_vote_admit(&fresh, 12, 1, &epoch(1, 3), &epoch(2, 0)));
    }

    // r[verify domain-primitives.raft-consensus.log-entry]
    #[test]
    fn log_entry_bounds_reject_zero_and_out_of_range() {
        assert!(raft_log_entry_in_bounds(&indexed(1, 2, 99), 10, 5));
        assert!(!raft_log_entry_in_bounds(&indexed(0, 2, 99), 10, 5));
        assert!(!raft_log_entry_in_bounds(&indexed(11, 2, 99), 10, 5));
        assert!(!raft_log_entry_in_bounds(&indexed(1, 6, 99), 10, 5));
    }

    // r[verify domain-primitives.raft-consensus.log-prefix]
    #[test]
    fn no_gap_rejects_skipped_index() {
        let contiguous = vec![indexed(1, 1, 10), indexed(2, 1, 20), indexed(3, 2, 30)];
        let gap = vec![indexed(1, 1, 10), indexed(3, 2, 30)];
        assert!(raft_log_no_gap(&contiguous));
        assert!(!raft_log_no_gap(&gap));
    }

    // r[verify domain-primitives.raft-consensus.log-prefix]
    #[test]
    fn prefix_and_append_preserve_contiguous_log() {
        let mut log = vec![indexed(1, 1, 10), indexed(2, 1, 20)];
        let prefix = vec![indexed(1, 1, 10)];
        assert!(raft_log_prefix(&prefix, &log));
        raft_log_append(&mut log, indexed(3, 2, 30));
        assert!(raft_log_no_gap(&log));
        assert!(raft_log_prefix(&prefix, &log));
    }

    // r[verify domain-primitives.raft-consensus.append-entries]
    #[test]
    fn append_entries_admission_checks_term_and_prev_log() {
        let follower_log = vec![log_entry(1, 10), log_entry(2, 20)];
        assert!(raft_append_entries_admit(2, 2, &follower_log, 2, 2));
        assert!(!raft_append_entries_admit(3, 2, &follower_log, 2, 2));
        assert!(!raft_append_entries_admit(2, 2, &follower_log, 2, 9));
    }

    // r[verify domain-primitives.raft-consensus.append-entries]
    #[test]
    fn append_entries_conflict_metadata_is_bounded() {
        let bounded = RaftAppendConflictMetadata {
            conflict_index: 2,
            conflict_term: 5,
            follower_log_len: 3,
        };
        assert!(raft_append_conflict_metadata_valid(&bounded));

        let absent = RaftAppendConflictMetadata {
            conflict_index: 0,
            conflict_term: 0,
            follower_log_len: 3,
        };
        assert!(raft_append_conflict_metadata_valid(&absent));

        let bad_absent = RaftAppendConflictMetadata {
            conflict_index: 0,
            conflict_term: 5,
            follower_log_len: 3,
        };
        assert!(!raft_append_conflict_metadata_valid(&bad_absent));

        let out_of_bounds = RaftAppendConflictMetadata {
            conflict_index: 4,
            conflict_term: 5,
            follower_log_len: 3,
        };
        assert!(!raft_append_conflict_metadata_valid(&out_of_bounds));
    }

    // r[verify domain-primitives.raft-consensus.follower-log-update]
    #[test]
    fn follower_log_update_metadata_preserves_rejected_and_accepted_lengths() {
        let rejected = RaftFollowerLogUpdateMetadata {
            prev_log_index: 2,
            appended_len: 1,
            successor_len: 3,
            rejected: true,
        };
        assert!(raft_follower_log_update_metadata_valid(&rejected, 3));

        let unsafe_mutation = RaftFollowerLogUpdateMetadata {
            successor_len: 4,
            ..rejected
        };
        assert!(!raft_follower_log_update_metadata_valid(&unsafe_mutation, 3));

        let accepted = RaftFollowerLogUpdateMetadata {
            prev_log_index: 2,
            appended_len: 2,
            successor_len: 4,
            rejected: false,
        };
        assert!(raft_follower_log_update_metadata_valid(&accepted, 3));

        let prefix_out_of_bounds = RaftFollowerLogUpdateMetadata {
            prev_log_index: 4,
            appended_len: 1,
            successor_len: 5,
            rejected: false,
        };
        assert!(!raft_follower_log_update_metadata_valid(&prefix_out_of_bounds, 3));

        let non_contiguous = RaftFollowerLogUpdateMetadata {
            prev_log_index: 2,
            appended_len: 2,
            successor_len: 5,
            rejected: false,
        };
        assert!(!raft_follower_log_update_metadata_valid(&non_contiguous, 3));
    }

    // r[verify domain-primitives.raft-consensus.log-matching]
    #[test]
    fn log_matching_requires_equal_prefix_terms() {
        let leader_log = vec![log_entry(1, 10), log_entry(2, 20), log_entry(3, 30)];
        let matching = vec![log_entry(1, 90), log_entry(2, 80), log_entry(4, 70)];
        let divergent = vec![log_entry(1, 90), log_entry(5, 80), log_entry(3, 70)];

        assert!(raft_log_matching_at(&leader_log, &matching, 2));
        assert!(!raft_log_matching_at(&leader_log, &matching, 3));
        assert!(!raft_log_matching_at(&leader_log, &divergent, 3));
        assert!(!raft_log_matching_at(&leader_log, &matching, 0));
    }

    // r[verify domain-primitives.raft-consensus.commit]
    #[test]
    fn quorum_commit_admission_is_monotone_and_bounded() {
        assert!(raft_quorum_commit_admit(3, 5, 2, 4, 6));
        assert_eq!(raft_commit_index_update(3, 5, 2, 4, 6), 4);

        assert!(!raft_quorum_commit_admit(2, 5, 2, 4, 6));
        assert_eq!(raft_commit_index_update(2, 5, 2, 4, 6), 2);

        assert!(!raft_quorum_commit_admit(3, 5, 4, 3, 6));
        assert_eq!(raft_commit_index_update(3, 5, 4, 3, 6), 4);

        assert!(!raft_quorum_commit_admit(3, 5, 2, 7, 6));
        assert_eq!(raft_commit_index_update(3, 5, 2, 7, 6), 2);
    }

    // r[verify domain-primitives.raft-consensus.leader-completeness]
    #[test]
    fn leader_completeness_requires_committed_entry_presence() {
        let committed = vec![indexed(1, 1, 10), indexed(2, 2, 20), indexed(3, 2, 30)];
        let complete_leader = vec![indexed(1, 1, 10), indexed(2, 2, 20), indexed(3, 3, 99)];
        let missing = vec![indexed(1, 1, 10), indexed(2, 3, 21), indexed(3, 3, 99)];

        assert!(raft_leader_contains_committed_entry(&committed, &complete_leader, 1));
        assert!(!raft_leader_contains_committed_entry(&committed, &missing, 1));
        assert!(!raft_leader_contains_committed_entry(&committed, &complete_leader, 4));
    }

    // r[verify domain-primitives.raft-consensus.state-machine-safety]
    #[test]
    fn state_machine_safety_rejects_divergent_applied_commands() {
        let a = vec![indexed(1, 1, 10), indexed(2, 2, 20)];
        let same = vec![indexed(1, 1, 10), indexed(2, 3, 20)];
        let divergent = vec![indexed(1, 1, 10), indexed(2, 3, 21)];

        assert!(raft_state_machine_safety_at(&a, &same, 2, 2, 1));
        assert!(!raft_state_machine_safety_at(&a, &divergent, 2, 2, 1));
        assert!(!raft_state_machine_safety_at(&a, &same, 1, 2, 1));
    }

    // r[verify domain-primitives.raft-consensus.message]
    #[test]
    fn message_envelope_admission_rejects_stale_or_invalid_inputs() {
        let msg = RaftMessageEnvelope {
            kind: RAFT_MESSAGE_APPEND_ENTRIES,
            term: 5,
            source: 1,
            target: 2,
            index: 7,
            payload: 9,
        };
        assert!(raft_message_envelope_valid(&msg, 3));
        assert!(raft_message_envelope_admit(4, &msg, 3));
        assert!(!raft_message_envelope_admit(6, &msg, 3));

        let invalid_kind = RaftMessageEnvelope { kind: 99, ..msg };
        assert!(!raft_message_envelope_valid(&invalid_kind, 3));
        let invalid_target = RaftMessageEnvelope { target: 4, ..msg };
        assert!(!raft_message_envelope_valid(&invalid_target, 3));
    }

    // r[verify domain-primitives.raft-consensus.trace]
    #[test]
    fn transition_trace_admits_fresh_steps_and_safe_stale_rejections() {
        let msg = RaftMessageEnvelope {
            kind: RAFT_MESSAGE_REQUEST_VOTE,
            term: 8,
            source: 1,
            target: 2,
            index: 0,
            payload: 1,
        };
        let admitted = RaftTransitionTraceStep {
            before_term: 7,
            after_term: 8,
            message: msg,
            admitted: true,
        };
        assert!(raft_transition_trace_step_admit(&admitted, 3));

        let stale = RaftTransitionTraceStep {
            before_term: 9,
            after_term: 9,
            message: msg,
            admitted: false,
        };
        assert!(raft_transition_trace_step_admit(&stale, 3));

        let mutating_stale = RaftTransitionTraceStep { after_term: 8, ..stale };
        assert!(!raft_transition_trace_step_admit(&mutating_stale, 3));
    }

    // r[verify domain-primitives.raft-consensus.membership]
    #[test]
    fn membership_joint_quorum_requires_old_and_new_majorities() {
        let joint = RaftMembershipConfig {
            epoch: 8,
            old_voter_count: 3,
            new_voter_count: 5,
            is_joint: true,
            max_node_id: 5,
        };
        assert!(raft_membership_config_valid(&joint));
        assert!(raft_joint_quorum_admit(&joint, 2, 3));
        assert!(!raft_joint_quorum_admit(&joint, 1, 3));
        assert!(!raft_joint_quorum_admit(&joint, 2, 2));

        let stable = RaftMembershipConfig {
            epoch: 9,
            old_voter_count: 5,
            new_voter_count: 5,
            is_joint: false,
            max_node_id: 5,
        };
        assert!(raft_membership_epoch_advance_admit(&joint, &stable));
        assert!(raft_joint_quorum_admit(&stable, 3, 0));
        assert!(!raft_joint_quorum_admit(&stable, 2, 5));
    }

    // r[verify domain-primitives.raft-consensus.membership]
    #[test]
    fn membership_epoch_admission_rejects_regression_and_invalid_stable_shape() {
        let stable = RaftMembershipConfig {
            epoch: 4,
            old_voter_count: 3,
            new_voter_count: 3,
            is_joint: false,
            max_node_id: 3,
        };
        let regressed = RaftMembershipConfig { epoch: 3, ..stable };
        assert!(!raft_membership_epoch_advance_admit(&stable, &regressed));

        let invalid_stable = RaftMembershipConfig {
            epoch: 4,
            old_voter_count: 3,
            new_voter_count: 5,
            is_joint: false,
            max_node_id: 5,
        };
        assert!(!raft_membership_config_valid(&invalid_stable));
        assert!(!raft_membership_epoch_advance_admit(&stable, &invalid_stable));
    }

    // r[verify domain-primitives.raft-consensus.snapshot]
    // r[verify domain-primitives.raft-consensus.install-snapshot]
    #[test]
    fn snapshot_admission_requires_newer_matching_boundary() {
        let log = vec![indexed(1, 1, 10), indexed(2, 2, 20), indexed(3, 2, 30)];
        let snapshot = RaftSnapshotMetadata {
            last_included_index: 2,
            last_included_term: 2,
            config_epoch: 7,
            state_machine_summary: 99,
        };
        assert!(raft_snapshot_metadata_valid(&snapshot));
        assert!(raft_snapshot_matches_prefix(&log, &snapshot));
        assert!(raft_install_snapshot_admit(3, 3, 1, &log, &snapshot));
        assert!(!raft_install_snapshot_admit(4, 3, 1, &log, &snapshot));
        assert!(!raft_install_snapshot_admit(3, 3, 2, &log, &snapshot));

        let mismatch = RaftSnapshotMetadata {
            last_included_index: 2,
            last_included_term: 9,
            config_epoch: 7,
            state_machine_summary: 99,
        };
        assert!(!raft_snapshot_matches_prefix(&log, &mismatch));
        assert!(!raft_install_snapshot_admit(3, 3, 1, &log, &mismatch));
    }

    fn invariant_bundle(term: u64, log_len: u64) -> RaftInvariantBundleMetadata {
        RaftInvariantBundleMetadata {
            current_term: term,
            persistent: RaftPersistentMetadata {
                current_term: term,
                voted_for: 1,
            },
            volatile: RaftVolatileMetadata {
                last_applied: 0,
                commit_index: 0,
                last_log_index: log_len,
            },
            snapshot: RaftSnapshotMetadata {
                last_included_index: 0,
                last_included_term: 0,
                config_epoch: 1,
                state_machine_summary: 0,
            },
            membership: RaftMembershipConfig {
                epoch: 1,
                old_voter_count: 3,
                new_voter_count: 3,
                is_joint: false,
                max_node_id: 3,
            },
            max_node_id: 3,
        }
    }

    // r[verify domain-primitives.raft-consensus.invariant-bundle]
    #[test]
    fn invariant_bundle_composes_metadata_and_log_checks() {
        let log = vec![indexed(1, 1, 10), indexed(2, 2, 20)];
        let bundle = invariant_bundle(2, 2);
        assert!(raft_invariant_bundle_valid(&bundle, &log));

        let bad_log_len = invariant_bundle(2, 3);
        assert!(!raft_invariant_bundle_valid(&bad_log_len, &log));

        let bad_gap = vec![indexed(1, 1, 10), indexed(3, 2, 20)];
        assert!(!raft_invariant_bundle_valid(&bundle, &bad_gap));
    }

    // r[verify domain-primitives.raft-consensus.invariant-bundle]
    #[test]
    fn invariant_preservation_requires_valid_after_bundle() {
        let log = vec![indexed(1, 1, 10), indexed(2, 2, 20)];
        let before = invariant_bundle(2, 2);
        let after = RaftInvariantBundleMetadata {
            current_term: 3,
            persistent: RaftPersistentMetadata {
                current_term: 3,
                voted_for: 0,
            },
            ..before
        };
        assert!(raft_invariant_preservation_admit(&before, &after, &log, &log));

        let invalid_after = RaftInvariantBundleMetadata {
            current_term: 3,
            persistent: RaftPersistentMetadata {
                current_term: 2,
                voted_for: 0,
            },
            ..before
        };
        assert!(!raft_invariant_preservation_admit(&before, &invalid_after, &log, &log));
    }

    // r[verify domain-primitives.raft-consensus.admission]
    #[test]
    fn deterministic_transition_admission_reports_first_failure() {
        let log = vec![indexed(1, 1, 10), indexed(2, 2, 20)];
        let before = invariant_bundle(2, 2);
        let after = RaftInvariantBundleMetadata {
            current_term: 3,
            persistent: RaftPersistentMetadata {
                current_term: 3,
                voted_for: 0,
            },
            ..before
        };
        let message = RaftMessageEnvelope {
            kind: RAFT_MESSAGE_APPEND_ENTRIES,
            term: 3,
            source: 1,
            target: 2,
            index: 2,
            payload: 20,
        };
        let step = RaftTransitionTraceStep {
            before_term: 2,
            after_term: 3,
            message,
            admitted: true,
        };

        let ok = raft_transition_admission_result(&before, &after, &log, &log, &step);
        assert!(ok.admitted);
        assert_eq!(ok.failing_check, RAFT_ADMISSION_OK);
        assert!(raft_transition_admitted(&before, &after, &log, &log, &step));

        let bad_before = RaftInvariantBundleMetadata {
            current_term: 2,
            persistent: RaftPersistentMetadata {
                current_term: 1,
                voted_for: 0,
            },
            ..before
        };
        let pre = raft_transition_admission_result(&bad_before, &after, &log, &log, &step);
        assert!(!pre.admitted);
        assert_eq!(pre.failing_check, RAFT_ADMISSION_BEFORE_INVARIANT_FAILED);

        let bad_message = RaftTransitionTraceStep {
            message: RaftMessageEnvelope { target: 9, ..message },
            ..step
        };
        let msg = raft_transition_admission_result(&before, &after, &log, &log, &bad_message);
        assert!(!msg.admitted);
        assert_eq!(msg.failing_check, RAFT_ADMISSION_MESSAGE_FAILED);

        let bad_trace = RaftTransitionTraceStep { after_term: 2, ..step };
        let trace = raft_transition_admission_result(&before, &after, &log, &log, &bad_trace);
        assert!(!trace.admitted);
        assert_eq!(trace.failing_check, RAFT_ADMISSION_TRACE_FAILED);

        let bad_after = RaftInvariantBundleMetadata {
            current_term: 3,
            persistent: RaftPersistentMetadata {
                current_term: 2,
                voted_for: 0,
            },
            ..before
        };
        let post = raft_transition_admission_result(&before, &bad_after, &log, &log, &step);
        assert!(!post.admitted);
        assert_eq!(post.failing_check, RAFT_ADMISSION_AFTER_INVARIANT_FAILED);
    }

    // r[verify domain-primitives.raft-consensus.openraft-log-pointers]
    #[test]
    fn openraft_log_pointer_lattice_accepts_ordered_and_rejects_regressions() {
        let valid = RaftOperationalLogPointers {
            purged: 1,
            snapshot: 2,
            applied: 3,
            committed: 4,
            flushed: 3,
            submitted: 5,
            accepted: 6,
        };
        assert!(raft_operational_log_pointers_valid(&valid));

        let applied_after_commit = RaftOperationalLogPointers {
            applied: 5,
            committed: 4,
            ..valid
        };
        assert!(!raft_operational_log_pointers_valid(&applied_after_commit));

        let accepted_before_submitted = RaftOperationalLogPointers { accepted: 4, ..valid };
        assert!(!raft_operational_log_pointers_valid(&accepted_before_submitted));
    }

    // r[verify domain-primitives.raft-consensus.openraft-io-progress]
    // r[verify domain-primitives.raft-consensus.openraft-vote-log-io]
    #[test]
    fn openraft_io_progress_and_vote_log_order_reject_reordering() {
        let old_progress = RaftIoProgressMetadata {
            flushed: 2,
            submitted: 4,
            accepted: 6,
        };
        let advanced = RaftIoProgressMetadata {
            flushed: 3,
            submitted: 5,
            accepted: 7,
        };
        assert!(raft_io_progress_valid(&old_progress));
        assert!(raft_io_progress_advance_admit(&old_progress, &advanced));

        let reordered_flush = RaftIoProgressMetadata {
            flushed: 6,
            submitted: 5,
            accepted: 7,
        };
        assert!(!raft_io_progress_valid(&reordered_flush));

        let backwards = RaftIoProgressMetadata {
            flushed: 1,
            submitted: 5,
            accepted: 7,
        };
        assert!(!raft_io_progress_advance_admit(&old_progress, &backwards));

        assert!(raft_vote_log_io_ordered(&RaftVoteLogIoMetadata {
            persisted_term: 5,
            log_term: 5,
        }));
        assert!(raft_vote_log_io_ordered(&RaftVoteLogIoMetadata {
            persisted_term: 6,
            log_term: 5,
        }));
        assert!(!raft_vote_log_io_ordered(&RaftVoteLogIoMetadata {
            persisted_term: 4,
            log_term: 5,
        }));
    }

    // r[verify domain-primitives.raft-consensus.openraft-effective-membership]
    // r[verify domain-primitives.raft-consensus.openraft-replication-session]
    #[test]
    fn effective_membership_and_replication_session_reject_stale_mutations() {
        let membership = RaftEffectiveMembershipMetadata {
            committed_epoch: 4,
            effective_epoch: 6,
        };
        assert!(raft_effective_membership_valid(&membership));
        let rolled_back = raft_effective_membership_rollback(&membership);
        assert_eq!(rolled_back.committed_epoch, 4);
        assert_eq!(rolled_back.effective_epoch, 4);
        assert!(raft_effective_membership_valid(&rolled_back));

        let invalid = RaftEffectiveMembershipMetadata {
            committed_epoch: 7,
            effective_epoch: 6,
        };
        assert!(!raft_effective_membership_valid(&invalid));

        let session = RaftReplicationSessionMetadata {
            leader_term: 8,
            leader_node: 1,
            membership_log_index: 10,
        };
        assert!(raft_replication_progress_update_admit(&session, &session, 3, 5, 9));
        assert!(!raft_replication_progress_update_admit(&session, &session, 6, 5, 9));
        assert!(!raft_replication_progress_update_admit(&session, &session, 3, 10, 9));

        let stale_leader = RaftReplicationSessionMetadata {
            leader_term: 7,
            ..session
        };
        assert!(!raft_replication_session_matches(&session, &stale_leader));
        assert!(!raft_replication_progress_update_admit(&session, &stale_leader, 3, 5, 9));

        let stale_membership = RaftReplicationSessionMetadata {
            membership_log_index: 9,
            ..session
        };
        assert!(!raft_replication_progress_update_admit(&session, &stale_membership, 3, 5, 9));
    }

    // r[verify domain-primitives.raft-consensus.openraft-read-linearizer]
    #[test]
    fn read_linearizer_ready_only_after_apply_catches_up() {
        assert!(raft_read_linearizer_ready(&RaftReadLinearizerMetadata {
            read_log_index: 5,
            last_applied_index: 5,
        }));
        assert!(raft_read_linearizer_ready(&RaftReadLinearizerMetadata {
            read_log_index: 5,
            last_applied_index: 7,
        }));
        assert!(!raft_read_linearizer_ready(&RaftReadLinearizerMetadata {
            read_log_index: 5,
            last_applied_index: 4,
        }));
    }

    // r[verify domain-primitives.raft-consensus.message]
    // r[verify domain-primitives.raft-consensus.admission]
    #[test]
    fn finite_kinds_and_admission_codes_cover_positive_negative_boundaries() {
        for kind in RAFT_MESSAGE_REQUEST_VOTE..=RAFT_MESSAGE_CLIENT_PROPOSAL {
            assert!(raft_message_kind_valid(kind));
        }
        assert!(!raft_message_kind_valid(0));
        assert!(!raft_message_kind_valid(RAFT_MESSAGE_CLIENT_PROPOSAL + 1));

        for code in RAFT_ADMISSION_OK..=RAFT_ADMISSION_AFTER_INVARIANT_FAILED {
            assert!(raft_admission_code_valid(code));
        }
        assert!(!raft_admission_code_valid(RAFT_ADMISSION_AFTER_INVARIANT_FAILED + 1));
    }

    // r[verify domain-primitives.raft-consensus.vote]
    // r[verify domain-primitives.raft-consensus.request-vote]
    // r[verify domain-primitives.raft-consensus.append-entries]
    #[test]
    fn vote_and_append_admission_have_positive_negative_mutation_evidence() {
        let fresh = VoteState {
            voted_term: 0,
            voted_for: 0,
        };
        assert!(request_vote_admit(&fresh, 2, 5, &epoch(5, 10), &epoch(5, 9)));
        assert!(!request_vote_admit(&fresh, 2, 4, &epoch(4, 10), &epoch(5, 9)));

        let already_voted = VoteState {
            voted_term: 5,
            voted_for: 2,
        };
        assert!(request_vote_admit(&already_voted, 2, 5, &epoch(5, 10), &epoch(5, 9)));
        assert!(!request_vote_admit(&already_voted, 3, 5, &epoch(5, 10), &epoch(5, 9)));

        let follower_log = vec![log_entry(1, 10), log_entry(2, 20), log_entry(4, 40)];
        assert!(raft_append_entries_admit(3, 4, &follower_log, 3, 4));
        assert!(!raft_append_entries_admit(5, 4, &follower_log, 3, 4));
        assert!(!raft_append_entries_admit(3, 4, &follower_log, 3, 3));
        assert!(!raft_append_entries_admit(3, 4, &follower_log, 4, 4));
    }

    // r[verify domain-primitives.raft-consensus.log-entry]
    // r[verify domain-primitives.raft-consensus.log-prefix]
    // r[verify domain-primitives.raft-consensus.log-matching]
    // r[verify domain-primitives.raft-consensus.leader-completeness]
    // r[verify domain-primitives.raft-consensus.state-machine-safety]
    #[test]
    fn log_and_state_safety_mutations_are_rejected() {
        let base = vec![indexed(1, 1, 10), indexed(2, 2, 20), indexed(3, 2, 30)];
        let same_prefix = vec![indexed(1, 1, 10), indexed(2, 2, 20)];
        let mutated_prefix = vec![indexed(1, 1, 10), indexed(2, 9, 20)];
        assert!(raft_log_prefix(&same_prefix, &base));
        assert!(!raft_log_prefix(&mutated_prefix, &base));

        let mut appended = same_prefix.clone();
        raft_log_append(&mut appended, indexed(3, 2, 30));
        assert_eq!(appended.len(), base.len());
        assert!(raft_log_no_gap(&appended));

        let leader_log = vec![log_entry(1, 10), log_entry(2, 20), log_entry(3, 30)];
        let matching_log = vec![log_entry(1, 99), log_entry(2, 98), log_entry(3, 97)];
        let mutated_log = vec![log_entry(1, 99), log_entry(7, 98), log_entry(3, 97)];
        assert!(raft_log_matching_at(&leader_log, &matching_log, 3));
        assert!(!raft_log_matching_at(&leader_log, &mutated_log, 3));

        let mutated_entry = vec![indexed(1, 1, 10), indexed(2, 2, 99), indexed(3, 2, 30)];
        assert!(raft_leader_contains_committed_entry(&base, &base, 1));
        assert!(!raft_leader_contains_committed_entry(&base, &mutated_entry, 1));
        assert!(raft_state_machine_safety_at(&base, &base, 3, 3, 2));
        assert!(!raft_state_machine_safety_at(&base, &mutated_entry, 3, 3, 1));
    }

    // r[verify domain-primitives.raft-consensus.snapshot]
    // r[verify domain-primitives.raft-consensus.install-snapshot]
    // r[verify domain-primitives.raft-consensus.membership]
    // r[verify domain-primitives.raft-consensus.invariant-bundle]
    #[test]
    fn snapshot_membership_and_invariant_mutations_are_rejected() {
        let log = vec![indexed(1, 1, 10), indexed(2, 2, 20), indexed(3, 3, 30)];
        let bundle = invariant_bundle(3, 3);
        assert!(raft_invariant_bundle_valid(&bundle, &log));

        let bad_persistent = RaftInvariantBundleMetadata {
            persistent: RaftPersistentMetadata {
                current_term: 2,
                voted_for: 1,
            },
            ..bundle
        };
        assert!(!raft_invariant_bundle_valid(&bad_persistent, &log));

        let bad_membership = RaftInvariantBundleMetadata {
            membership: RaftMembershipConfig {
                epoch: 1,
                old_voter_count: 3,
                new_voter_count: 4,
                is_joint: false,
                max_node_id: 4,
            },
            max_node_id: 4,
            ..bundle
        };
        assert!(!raft_invariant_bundle_valid(&bad_membership, &log));

        let snapshot = RaftSnapshotMetadata {
            last_included_index: 3,
            last_included_term: 3,
            config_epoch: 1,
            state_machine_summary: 77,
        };
        assert!(raft_snapshot_matches_prefix(&log, &snapshot));
        assert!(raft_install_snapshot_admit(3, 4, 2, &log, &snapshot));

        let mutated_snapshot = RaftSnapshotMetadata {
            last_included_term: 2,
            ..snapshot
        };
        assert!(!raft_snapshot_matches_prefix(&log, &mutated_snapshot));
        assert!(!raft_install_snapshot_admit(3, 4, 2, &log, &mutated_snapshot));

        let joint = RaftMembershipConfig {
            epoch: 2,
            old_voter_count: 3,
            new_voter_count: 5,
            is_joint: true,
            max_node_id: 5,
        };
        assert!(raft_joint_quorum_admit(&joint, 2, 3));
        assert!(!raft_joint_quorum_admit(&joint, 2, 2));
    }

    // r[verify domain-primitives.raft-consensus.trace]
    // r[verify domain-primitives.raft-consensus.admission]
    #[test]
    fn deterministic_admission_keeps_first_failure_under_multiple_mutations() {
        let log = vec![indexed(1, 1, 10), indexed(2, 2, 20)];
        let before = invariant_bundle(2, 2);
        let after = RaftInvariantBundleMetadata {
            current_term: 3,
            persistent: RaftPersistentMetadata {
                current_term: 3,
                voted_for: 0,
            },
            ..before
        };
        let message = RaftMessageEnvelope {
            kind: RAFT_MESSAGE_APPEND_ENTRIES,
            term: 3,
            source: 1,
            target: 2,
            index: 2,
            payload: 20,
        };
        let step = RaftTransitionTraceStep {
            before_term: 2,
            after_term: 3,
            message,
            admitted: true,
        };

        let bad_before = RaftInvariantBundleMetadata {
            current_term: 2,
            persistent: RaftPersistentMetadata {
                current_term: 1,
                voted_for: 9,
            },
            ..before
        };
        let bad_step = RaftTransitionTraceStep {
            after_term: 2,
            message: RaftMessageEnvelope { target: 9, ..message },
            ..step
        };
        let bad_after = RaftInvariantBundleMetadata {
            current_term: 3,
            persistent: RaftPersistentMetadata {
                current_term: 2,
                voted_for: 0,
            },
            ..before
        };

        let pre_first = raft_transition_admission_result(&bad_before, &bad_after, &log, &log, &bad_step);
        assert!(!pre_first.admitted);
        assert_eq!(pre_first.failing_check, RAFT_ADMISSION_BEFORE_INVARIANT_FAILED);

        let message_first = raft_transition_admission_result(&before, &bad_after, &log, &log, &bad_step);
        assert!(!message_first.admitted);
        assert_eq!(message_first.failing_check, RAFT_ADMISSION_MESSAGE_FAILED);

        let bad_trace = RaftTransitionTraceStep { after_term: 2, ..step };
        let trace_first = raft_transition_admission_result(&before, &bad_after, &log, &log, &bad_trace);
        assert!(!trace_first.admitted);
        assert_eq!(trace_first.failing_check, RAFT_ADMISSION_TRACE_FAILED);
    }
}
