//! Positive and negative shell tests for the final authorization
//! orchestration: ordered CID-bound proof resolution, preflight ordering,
//! one replay commit, live values, and redacted receipts.

use std::cell::Cell;
use std::collections::BTreeMap;

use super::*;
use crate::profile::wire_fields;

struct MemoryStore {
    envelopes: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl V1DelegationStore for MemoryStore {
    fn get(&self, cid: &[u8]) -> Result<Option<Vec<u8>>, V1DelegationStoreError> {
        Ok(self.envelopes.get(cid).cloned())
    }
}

struct UnavailableStore;

impl V1DelegationStore for UnavailableStore {
    fn get(&self, _cid: &[u8]) -> Result<Option<Vec<u8>>, V1DelegationStoreError> {
        Err(V1DelegationStoreError::Unavailable)
    }
}

struct NoRevocations;

impl V1RevocationObservation for NoRevocations {
    fn is_revoked(&self, _cid: &[u8]) -> bool {
        false
    }
}

struct RevokeAll;

impl V1RevocationObservation for RevokeAll {
    fn is_revoked(&self, _cid: &[u8]) -> bool {
        true
    }
}

#[derive(Default)]
struct CountingReplay {
    calls: Cell<usize>,
    admitted: Cell<bool>,
}

impl CountingReplay {
    fn allow() -> Self {
        Self {
            calls: Cell::new(0),
            admitted: Cell::new(true),
        }
    }
}

impl V1InvocationReplay for CountingReplay {
    fn admit(&self, _invocation_cid: &[u8], _command: &str, _subject: &str) -> Result<(), V1ReplayError> {
        self.calls.set(self.calls.get() + 1);
        if self.admitted.get() {
            Ok(())
        } else {
            Err(V1ReplayError::Denied)
        }
    }
}

struct KeyMap(BTreeMap<String, [u8; 32]>);

impl crate::profile::v1::DidKeyResolver for KeyMap {
    fn resolve_did_key(&self, did: &str) -> Result<[u8; 32], String> {
        self.0.get(did).copied().ok_or_else(|| format!("unresolved {did}"))
    }
}

fn signing_key(byte: u8) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[byte; 32])
}

fn did_of(key: &ed25519_dalek::SigningKey) -> String {
    crate::profile::v1::did_key_from_public_key(key.verifying_key().as_bytes()).expect("did projection")
}

fn delegation_envelope(
    issuer: &ed25519_dalek::SigningKey,
    audience: &str,
    subject: Option<&str>,
    command: Option<&str>,
    policy: Option<&serde_json::Value>,
    nbf: Option<i64>,
    exp: Option<i64>,
) -> (Vec<u8>, Vec<u8>) {
    let mut fields =
        wire_fields::delegation_payload(&did_of(issuer), audience, subject, command, policy, nbf, exp, &[]);
    let payload_bytes = crate::profile::v1::canonical_payload_bytes(&fields);
    let cid = crate::profile::v1::cid_v1_of_payload(&payload_bytes);
    let envelope = crate::profile::v1::sign_payload(&fields, issuer);
    let _ = &mut fields;
    (cid, envelope)
}

fn invocation_envelope(
    invoker: &ed25519_dalek::SigningKey,
    audience: &str,
    subject: &str,
    command: &str,
    proofs: &[Vec<u8>],
) -> (Vec<u8>, Vec<u8>) {
    let fields = wire_fields::invocation_payload(&did_of(invoker), audience, subject, command, proofs);
    let payload_bytes = crate::profile::v1::canonical_payload_bytes(&fields);
    let cid = crate::profile::v1::cid_v1_of_payload(&payload_bytes);
    let envelope = crate::profile::v1::sign_payload(&fields, invoker);
    (cid, envelope)
}

fn install(store: &mut MemoryStore, cid: Vec<u8>, envelope: Vec<u8>) {
    store.envelopes.insert(cid, envelope);
}

fn expect_allowed(outcome: AuthOutcome<'_>) -> (LiveAuthorization<'_>, AuthorizationReceipt) {
    match outcome {
        AuthOutcome::Allowed(live, receipt) => (live, receipt),
        AuthOutcome::Denied { reason, .. } => panic!("expected allowed outcome, got denial: {reason:?}"),
    }
}

fn expect_denied(outcome: AuthOutcome<'_>) -> (Option<String>, AuthorizationReceipt) {
    match outcome {
        AuthOutcome::Allowed(_, _) => panic!("expected denied outcome"),
        AuthOutcome::Denied { reason, receipt } => (reason, receipt),
    }
}

#[test]
fn direct_delegation_invocation_is_admitted() {
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, root_env) =
        delegation_envelope(&root, &did_of(&invoker), Some(subject_did), Some("/crud"), None, Some(1_000), Some(2_000));
    install(&mut store, root_cid.clone(), root_env);
    let (inv_cid, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let _ = &inv_cid;

    let replay = CountingReplay::allow();
    let outcome =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect("preflight must not hard-fail");
    let (live, receipt) = expect_allowed(outcome);
    assert_eq!(live.subject(), subject_did);
    assert_eq!(live.executor(), did_of(&invoker));
    assert_eq!(live.command().as_str(), "/crud/create");
    assert_eq!(live.root_issuer(), did_of(&root));
    assert_eq!(replay.calls.get(), 1);
    assert!(receipt.allowed);
    assert_eq!(receipt.replay_class.as_str(), "committed");
}

#[test]
fn delegated_two_hop_invocation_is_admitted() {
    let root = signing_key(1);
    let mid = signing_key(3);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, root_env) =
        delegation_envelope(&root, &did_of(&mid), Some(subject_did), Some("/"), None, Some(1_000), Some(2_000));
    install(&mut store, root_cid.clone(), root_env);
    let (mid_cid, mid_env) =
        delegation_envelope(&mid, &did_of(&invoker), None, Some("/crud"), None, Some(1_000), Some(2_000));
    install(&mut store, mid_cid.clone(), mid_env);
    let (_, inv_env) =
        invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid, mid_cid]);

    let mut keys = BTreeMap::new();
    for key in [&root, &mid, &invoker] {
        keys.insert(did_of(key), key.verifying_key().to_bytes());
    }
    let replay = CountingReplay::allow();
    let outcome =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect("preflight");
    let (live, _) = expect_allowed(outcome);
    assert_eq!(live.subject(), subject_did);
    assert_eq!(replay.calls.get(), 1);
}

#[test]
fn invalid_invocation_signature_fails_before_proof_loading_or_replay() {
    const FIRST_SIGNATURE_BYTE_INDEX: usize = 3;
    const SIGNATURE_TAMPER_MASK: u8 = 1;
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let (_, mut inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[]);
    assert!(inv_env.len() > FIRST_SIGNATURE_BYTE_INDEX);
    inv_env[FIRST_SIGNATURE_BYTE_INDEX] ^= SIGNATURE_TAMPER_MASK;
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let replay = CountingReplay::default();
    let error =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect_err("tampered Invocation must fail");
    assert_eq!(error, AuthError::InvalidInvocationSignature);
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn preflight_denial_calls_replay_zero_times() {
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    // Command mismatch: delegation grants /crypto, invocation wants /cryptocurrency.
    let (root_cid, root_env) = delegation_envelope(
        &root,
        &did_of(&invoker),
        Some(subject_did),
        Some("/crypto"),
        None,
        Some(1_000),
        Some(2_000),
    );
    install(&mut store, root_cid.clone(), root_env);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/cryptocurrency", &[root_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());

    let replay = CountingReplay::default();
    let outcome =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect("preflight");
    let (reason, receipt) = expect_denied(outcome);
    let _ = reason;
    assert!(!receipt.allowed);
    assert_eq!(replay.calls.get(), 0, "preflight denial must not call replay");
}

#[test]
fn executor_mismatch_denies_before_replay() {
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, root_env) =
        delegation_envelope(&root, &did_of(&invoker), Some(subject_did), Some("/crud"), None, Some(1_000), Some(2_000));
    install(&mut store, root_cid.clone(), root_env);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());

    let replay = CountingReplay::default();
    let outcome = authorize_invocation(
        &inv_env,
        &KeyMap(keys),
        &store,
        &NoRevocations,
        &replay,
        "did:key:z6Mkwrong-executor",
        1_500,
        60,
    )
    .expect("preflight");
    let (reason, receipt) = expect_denied(outcome);
    assert!(reason.is_some_and(|text| text.contains("executor")));
    assert!(!receipt.allowed);
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn missing_proof_fails_before_replay() {
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[vec![0xAA; 8]]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let replay = CountingReplay::default();
    let error =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect_err("missing proof must hard-fail");
    assert_eq!(error, AuthError::MissingProof);
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn unavailable_proof_store_fails_before_replay() {
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[vec![0xAA; 8]]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let replay = CountingReplay::default();
    let error = authorize_invocation(
        &inv_env,
        &KeyMap(keys),
        &UnavailableStore,
        &NoRevocations,
        &replay,
        &did_of(&invoker),
        1_500,
        60,
    )
    .expect_err("unavailable proof store must hard-fail");
    assert_eq!(error, AuthError::ProofStoreUnavailable);
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn malformed_proof_fails_before_replay() {
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let requested_cid = vec![0xBB; 8];
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    store.envelopes.insert(requested_cid.clone(), vec![0xFF; 16]);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[requested_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let replay = CountingReplay::default();
    let error =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect_err("malformed proof must hard-fail");
    assert!(matches!(error, AuthError::Wire(_)));
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn substituted_proof_is_rejected_with_binding_error() {
    let root = signing_key(1);
    let other = signing_key(2);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, _) =
        delegation_envelope(&root, &did_of(&invoker), Some(subject_did), Some("/crud"), None, Some(1_000), Some(2_000));
    // Store a DIFFERENT valid Delegation (different CID) under the requested proof.
    let (_, other_env) = delegation_envelope(
        &other,
        &did_of(&invoker),
        Some(subject_did),
        Some("/crud"),
        None,
        Some(1_000),
        Some(2_000),
    );
    install(&mut store, root_cid.clone(), other_env);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let mut keys = BTreeMap::new();
    for key in [&root, &other, &invoker] {
        keys.insert(did_of(key), key.verifying_key().to_bytes());
    }
    let replay = CountingReplay::default();
    let error =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect_err("substitution must fail");
    assert_eq!(error, AuthError::SubstituteProof);
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn revoked_delegation_denies_before_replay() {
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, root_env) =
        delegation_envelope(&root, &did_of(&invoker), Some(subject_did), Some("/crud"), None, Some(1_000), Some(2_000));
    install(&mut store, root_cid.clone(), root_env);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let replay = CountingReplay::default();
    let outcome =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &RevokeAll, &replay, &did_of(&invoker), 1_500, 60)
            .expect("preflight");
    let (_, receipt) = expect_denied(outcome);
    assert!(!receipt.allowed);
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn policy_denies_before_replay() {
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let policy = serde_json::json!([[["==", ".b", 2]]]);
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, root_env) = delegation_envelope(
        &root,
        &did_of(&invoker),
        Some(subject_did),
        Some("/crud"),
        Some(&policy),
        Some(1_000),
        Some(2_000),
    );
    install(&mut store, root_cid.clone(), root_env);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let replay = CountingReplay::default();
    let outcome =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect("preflight");
    let (_, receipt) = expect_denied(outcome);
    assert!(!receipt.allowed);
    assert_eq!(replay.calls.get(), 0);
}

#[test]
fn replay_denial_produces_denied_outcome_with_one_call() {
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, root_env) =
        delegation_envelope(&root, &did_of(&invoker), Some(subject_did), Some("/crud"), None, Some(1_000), Some(2_000));
    install(&mut store, root_cid.clone(), root_env);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let replay = CountingReplay::default();
    let outcome =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect("preflight");
    let (reason, receipt) = expect_denied(outcome);
    assert!(reason.is_some());
    assert!(!receipt.allowed);
    assert_eq!(receipt.replay_class, "denied");
    assert_eq!(receipt.revocation_class, "observed-not-revoked");
    assert_eq!(replay.calls.get(), 1);
}

#[test]
fn receipts_are_deterministic_redacted_and_not_live() {
    const BLAKE3_DIGEST_BYTES: usize = 32;
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    let (root_cid, root_env) =
        delegation_envelope(&root, &did_of(&invoker), Some(subject_did), Some("/crud"), None, Some(1_000), Some(2_000));
    install(&mut store, root_cid.clone(), root_env);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());

    let replay1 = CountingReplay::allow();
    let replay2 = CountingReplay::allow();
    let outcome1 = authorize_invocation(
        &inv_env,
        &KeyMap(keys.clone()),
        &store,
        &NoRevocations,
        &replay1,
        &did_of(&invoker),
        1_500,
        60,
    )
    .expect("preflight");
    let outcome2 =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay2, &did_of(&invoker), 1_500, 60)
            .expect("preflight");
    let (live, receipt1) = expect_allowed(outcome1);
    let (_, receipt2) = expect_allowed(outcome2);

    assert_eq!(receipt_blake3(&receipt1), receipt_blake3(&receipt2), "receipts must be deterministic");
    let mut substituted_profile = receipt1.clone();
    "onix-compact-ed25519-v1".clone_into(&mut substituted_profile.profile);
    assert_ne!(receipt_blake3(&receipt1), receipt_blake3(&substituted_profile));
    let mut substituted_cohort = receipt1.clone();
    "different-spec-cohort".clone_into(&mut substituted_cohort.spec_cohort);
    assert_ne!(receipt_blake3(&receipt1), receipt_blake3(&substituted_cohort));
    assert_eq!(receipt1.command_identity.len(), BLAKE3_DIGEST_BYTES);
    assert_eq!(receipt1.subject_identity.len(), BLAKE3_DIGEST_BYTES);
    assert_ne!(receipt1.command_identity, b"/crud/create");
    assert_ne!(receipt1.subject_identity, subject_did.as_bytes());
    assert_eq!(live.command().as_str(), "/crud/create");
    // Redaction: the receipt serializes to JSON that omits the raw token
    // bytes (they are large multi-hundred-byte envelopes) and carries classes.
    let serialized = serde_json::to_string(&receipt1).expect("serialize");
    assert!(!serialized.contains("did:key:z6Mk"));
    assert!(serialized.contains("revocation_class"));
    assert!(serialized.contains("replay_class"));
    // Non-authority: no public constructor can rebuild live authorization
    // (a borrowing value with private fields); receipts are evidence only.
    let _ = receipt1.limits.max_proof_count;
}

#[test]
fn oversized_proof_fails_before_replay() {
    let root = signing_key(1);
    let invoker = signing_key(7);
    let subject_did = "did:key:z6Mksubject";
    let (root_cid, root_env) =
        delegation_envelope(&root, &did_of(&invoker), Some(subject_did), Some("/crud"), None, Some(1_000), Some(2_000));
    let mut store = MemoryStore {
        envelopes: BTreeMap::new(),
    };
    // Pad the stored envelope past the byte limit.
    let mut padded = root_env;
    padded.extend(std::iter::repeat_n(0_u8, MAX_DELEGATION_BYTES + 1));
    install(&mut store, root_cid.clone(), padded);
    let (_, inv_env) = invocation_envelope(&invoker, &did_of(&invoker), subject_did, "/crud/create", &[root_cid]);
    let mut keys = BTreeMap::new();
    keys.insert(did_of(&root), root.verifying_key().to_bytes());
    keys.insert(did_of(&invoker), invoker.verifying_key().to_bytes());
    let replay = CountingReplay::default();
    let error =
        authorize_invocation(&inv_env, &KeyMap(keys), &store, &NoRevocations, &replay, &did_of(&invoker), 1_500, 60)
            .expect_err("oversized proof must fail");
    assert_eq!(error, AuthError::OversizedProof);
    assert_eq!(replay.calls.get(), 0);
}
