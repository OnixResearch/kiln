#![cfg(feature = "status-native-fixtures")]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use kiln_adapter_cob::dispatch_mutation;
use kiln_adapter_radicle::mutation::{
    DeclaredMutationTarget, PATCH_SCHEMA_LINE, PATCH_TYPE_NAME, PatchReviewAction, PatchVerdict,
    RadicleMutationProfile, RadicleMutationTransport, operation_identity,
};
use kiln_adapter_radicle::status::{
    PINNED_SECRETSPEC_VERSION, PinnedSecretSpecExecutable, ResolvedSigner, SecretSpecDeclaration,
};
use kiln_core::cob::{CobCapability, CobEffectPlan, CobProfile};
use kiln_core::types::{
    CobObjectId, CobProfileId, ContentId, EffectId, ExternalRef, PolicyId, ProviderProfileId,
};
use kiln_provider_api::{CollaborationMutationOutcome, CollaborationMutationPort, PortFailure};
use nonempty::NonEmpty;
use onix_radicle_cob_git_store::GitCobStore;
use radicle_cob::object::collaboration;
use radicle_cob::{Create, Entry, ObjectId, TypeName, Update, Version};
use radicle_crypto::PublicKey;

const FIXTURE_RID: &str = "rad:zMutationFixture";
const PRIMARY_PROVIDER: &str = "fixture-primary";
const MISSING_PROVIDER: &str = "fixture-missing";
const PRIMARY_SEED: &str = "0101010101010101010101010101010101010101010101010101010101010101";
const SECONDARY_SEED: &str = "0202020202020202020202020202020202020202020202020202020202020202";
const BASE_OID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MAPPER_VERSION: u16 = 1;
const MAX_HEADS: usize = 1;
const TIMEOUT_DEADLINE_MS: u64 = 50;
const TIMEOUT_DELAY_MS: u64 = 100;
#[cfg(unix)]
const READ_EXECUTE_MODE: u32 = 0o500;
#[cfg(unix)]
const OWNER_FULL_MODE: u32 = 0o700;

struct Fixture {
    temporary: tempfile::TempDir,
    executable: PathBuf,
    object_id: String,
    revision_id: String,
    head_id: String,
    resource_id: String,
    signer: ResolvedSigner,
}

fn external(namespace: &str, kind: &str, id: &str) -> ExternalRef {
    ExternalRef::new(namespace, kind, id).unwrap_or_else(|_| unreachable!())
}

fn executable_identity(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|_| unreachable!());
    blake3::hash(&bytes).to_string()
}

fn initialize_fixture() -> Fixture {
    let temporary = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let storage_root = temporary.path().join("storage");
    std::fs::create_dir_all(&storage_root).unwrap_or_else(|_| unreachable!());
    let repository_path = storage_root.join(FIXTURE_RID);
    let repository =
        cob_git2::Repository::init_bare(&repository_path).unwrap_or_else(|_| unreachable!());
    let mut config = repository.config().unwrap_or_else(|_| unreachable!());
    config
        .set_str("user.name", "Kiln Mutation Fixture")
        .unwrap_or_else(|_| unreachable!());
    config
        .set_str("user.email", "mutation-fixture@kiln.invalid")
        .unwrap_or_else(|_| unreachable!());

    let signature = cob_git2::Signature::now("Fixture", "fixture@kiln.invalid")
        .unwrap_or_else(|_| unreachable!());
    let tree_builder = repository
        .treebuilder(None)
        .unwrap_or_else(|_| unreachable!());
    let tree_id = tree_builder.write().unwrap_or_else(|_| unreachable!());
    let tree = repository
        .find_tree(tree_id)
        .unwrap_or_else(|_| unreachable!());
    let resource = repository
        .commit(
            None,
            &signature,
            &signature,
            "fixture identity resource",
            &tree,
            &[],
        )
        .unwrap_or_else(|_| unreachable!());

    let signer =
        ResolvedSigner::from_resolved_secret(PRIMARY_SEED).unwrap_or_else(|_| unreachable!());
    let namespace = PublicKey::from_str(signer.public_key()).unwrap_or_else(|_| unreachable!());
    let store = GitCobStore::new(&repository);
    let revision = format!(
        "{{\"type\":\"revision\",\"description\":\"Fixture revision\",\"base\":\"{BASE_OID}\",\"oid\":\"{BASE_OID}\"}}"
    )
    .into_bytes();
    let edit = br#"{"type":"edit","title":"Fixture patch","target":"delegates"}"#.to_vec();
    let mut contents = NonEmpty::new(revision);
    contents.tail.push(edit);
    let created = collaboration::create::<NonEmpty<Entry>, _>(
        &store,
        signer.signing_key(),
        Some(resource.into()),
        Vec::new(),
        &namespace,
        Create {
            contents,
            type_name: TypeName::from_str(PATCH_TYPE_NAME).unwrap_or_else(|_| unreachable!()),
            message: "fixture patch root".to_string(),
            embeds: Vec::new(),
            version: Version::default(),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let object_id = created.id().to_string();
    let revision_id = object_id.clone();
    let head_id = object_id.clone();
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_secretspec_fixture"));
    Fixture {
        temporary,
        executable,
        object_id,
        revision_id,
        head_id,
        resource_id: resource.to_string(),
        signer,
    }
}

fn profile(fixture: &Fixture, provider: &str) -> RadicleMutationProfile {
    let action = PatchReviewAction {
        revision: fixture.revision_id.clone(),
        verdict: PatchVerdict::Accept,
        summary: "Kiln checks passed".to_string(),
    };
    let identity = operation_identity(&action).unwrap_or_else(|_| unreachable!());
    RadicleMutationProfile::pinned(
        BTreeSet::from([DeclaredMutationTarget {
            repository_rid: FIXTURE_RID.to_string(),
            object_type: PATCH_TYPE_NAME.to_string(),
            object_id: fixture.object_id.clone(),
        }]),
        BTreeMap::from([(identity, action)]),
        SecretSpecDeclaration {
            project_root: fixture.temporary.path().to_path_buf(),
            project: "kiln-mutation-fixture".to_string(),
            profile: "fixture".to_string(),
            secret_key: "RADICLE_SIGNER_SEED".to_string(),
            provider: Some(provider.to_string()),
        },
        PinnedSecretSpecExecutable {
            path: fixture.executable.clone(),
            blake3: executable_identity(&fixture.executable),
            version: PINNED_SECRETSPEC_VERSION.to_string(),
        },
    )
}

fn cob_profile(profile: &RadicleMutationProfile) -> CobProfile {
    CobProfile {
        id: profile.id.clone(),
        provider: ProviderProfileId::new("radicle-mutation").unwrap_or_else(|_| unreachable!()),
        type_name: external("radicle", "cob-type", PATCH_TYPE_NAME),
        schema_line: external("radicle", "schema", PATCH_SCHEMA_LINE),
        mapper_version: MAPPER_VERSION,
        capabilities: BTreeSet::from([CobCapability::Review]),
        max_heads: MAX_HEADS,
        max_payload_bytes: profile.max_payload_bytes,
        complete_head_resolution: true,
    }
}

fn plan(fixture: &Fixture, profile: &RadicleMutationProfile, head: &str) -> CobEffectPlan {
    let identity = profile
        .operations
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    CobEffectPlan {
        effect_id: EffectId::new("patch-review-effect").unwrap_or_else(|_| unreachable!()),
        profile_id: CobProfileId::new(profile.id.as_str()).unwrap_or_else(|_| unreachable!()),
        repository: external("radicle", "repository", FIXTURE_RID),
        object_id: CobObjectId::new(&fixture.object_id).unwrap_or_else(|_| unreachable!()),
        expected_heads: BTreeSet::from([external("radicle", "operation", head)]),
        action: CobCapability::Review,
        payload_identity: ContentId::from_blake3_hex(identity).unwrap_or_else(|_| unreachable!()),
        actor: external("radicle", "node", fixture.signer.public_key()),
        policy_id: PolicyId::new("single-writer").unwrap_or_else(|_| unreachable!()),
        idempotency_key: external("kiln", "operation", "patch-review-one"),
    }
}

fn storage_root(fixture: &Fixture) -> PathBuf {
    fixture.temporary.path().join("storage")
}

fn append_foreign_head(fixture: &Fixture) -> String {
    let repository = cob_git2::Repository::open(storage_root(fixture).join(FIXTURE_RID))
        .unwrap_or_else(|_| unreachable!());
    let secondary =
        ResolvedSigner::from_resolved_secret(SECONDARY_SEED).unwrap_or_else(|_| unreachable!());
    let primary_namespace =
        PublicKey::from_str(fixture.signer.public_key()).unwrap_or_else(|_| unreachable!());
    let namespace = PublicKey::from_str(secondary.public_key()).unwrap_or_else(|_| unreachable!());
    let store = GitCobStore::new(&repository);
    let payload = format!(
        "{{\"type\":\"review\",\"revision\":\"{}\",\"summary\":\"Foreign review\",\"verdict\":\"reject\"}}",
        fixture.revision_id
    )
    .into_bytes();
    let updated = collaboration::update::<NonEmpty<Entry>, _>(
        &store,
        secondary.signing_key(),
        Some(radicle_oid::Oid::from_str(&fixture.resource_id).unwrap_or_else(|_| unreachable!())),
        Vec::new(),
        &namespace,
        Update {
            changes: NonEmpty::new(payload),
            object_id: ObjectId::from_str(&fixture.object_id).unwrap_or_else(|_| unreachable!()),
            type_name: TypeName::from_str(PATCH_TYPE_NAME).unwrap_or_else(|_| unreachable!()),
            message: "foreign fixture review".to_string(),
            embeds: Vec::new(),
        },
    )
    .unwrap_or_else(|_| unreachable!());
    let type_name = TypeName::from_str(PATCH_TYPE_NAME).unwrap_or_else(|_| unreachable!());
    let object_id = ObjectId::from_str(&fixture.object_id).unwrap_or_else(|_| unreachable!());
    let primary_ref = store
        .ref_name(&primary_namespace, &type_name, &object_id)
        .unwrap_or_else(|_| unreachable!());
    repository
        .find_reference(&primary_ref)
        .unwrap_or_else(|_| unreachable!())
        .delete()
        .unwrap_or_else(|_| unreachable!());
    updated.head.to_string()
}

fn applied(outcome: CollaborationMutationOutcome) -> kiln_core::cob::CobObservation {
    assert!(
        matches!(outcome, CollaborationMutationOutcome::Applied(_)),
        "expected applied mutation, got {outcome:?}"
    );
    match outcome {
        CollaborationMutationOutcome::Applied(observation) => *observation,
        _ => unreachable!(),
    }
}

fn port_error<T>(result: Result<T, PortFailure>) -> PortFailure {
    match result {
        Err(error) => error,
        Ok(_) => unreachable!(),
    }
}

fn domain_code(error: PortFailure) -> String {
    match error {
        PortFailure::Domain(diagnostic) => diagnostic.code,
        _ => unreachable!(),
    }
}

// r[verify kiln.radicle.shell_mutation.scenario.authorized]
// r[verify kiln.radicle.mutation_scope.scenario.owned]
// r[verify kiln.radicle.mutation_conformance]
// r[verify kiln.radicle.mutation_conformance.scenario.repeat]
#[test]
fn authorized_review_repeats_without_a_second_write() {
    let fixture = initialize_fixture();
    let profile = profile(&fixture, PRIMARY_PROVIDER);
    let cob_profile = cob_profile(&profile);
    let mut transport = RadicleMutationTransport::new_fixture(
        storage_root(&fixture),
        profile.clone(),
        Duration::ZERO,
    )
    .unwrap_or_else(|_| unreachable!());
    let first_plan = plan(&fixture, &profile, &fixture.head_id);
    let expected = transport
        .observe(&first_plan)
        .unwrap_or_else(|_| unreachable!());
    let first = applied(
        dispatch_mutation(&cob_profile, &expected, first_plan, &mut transport)
            .unwrap_or_else(|_| unreachable!()),
    );

    let repeat_plan = plan(
        &fixture,
        &profile,
        first
            .heads
            .iter()
            .next()
            .map(|head| head.native_id.as_str())
            .unwrap_or_else(|| unreachable!()),
    );
    let repeated = applied(
        dispatch_mutation(&cob_profile, &first, repeat_plan, &mut transport)
            .unwrap_or_else(|_| unreachable!()),
    );
    assert_eq!(repeated, first);
}

#[test]
fn moved_head_is_rejected_before_a_second_signer_resolution() {
    let fixture = initialize_fixture();
    let profile = profile(&fixture, PRIMARY_PROVIDER);
    let cob_profile = cob_profile(&profile);
    let mut transport = RadicleMutationTransport::new_fixture(
        storage_root(&fixture),
        profile.clone(),
        Duration::ZERO,
    )
    .unwrap_or_else(|_| unreachable!());
    let stale_plan = plan(&fixture, &profile, &fixture.head_id);
    let stale_expected = transport
        .observe(&stale_plan)
        .unwrap_or_else(|_| unreachable!());
    let _ = dispatch_mutation(
        &cob_profile,
        &stale_expected,
        stale_plan.clone(),
        &mut transport,
    )
    .unwrap_or_else(|_| unreachable!());
    let error = port_error(dispatch_mutation(
        &cob_profile,
        &stale_expected,
        stale_plan,
        &mut transport,
    ));
    assert_eq!(domain_code(error), "stale_cob_preflight");
}

// r[verify kiln.radicle.shell_mutation.scenario.ambient]
#[test]
fn unresolved_signer_stops_before_preview_and_write() {
    let fixture = initialize_fixture();
    let profile = profile(&fixture, MISSING_PROVIDER);
    let cob_profile = cob_profile(&profile);
    let mut transport = RadicleMutationTransport::new_fixture(
        storage_root(&fixture),
        profile.clone(),
        Duration::ZERO,
    )
    .unwrap_or_else(|_| unreachable!());
    let plan = plan(&fixture, &profile, &fixture.head_id);
    let expected = transport.observe(&plan).unwrap_or_else(|_| unreachable!());
    let error = port_error(dispatch_mutation(
        &cob_profile,
        &expected,
        plan,
        &mut transport,
    ));
    let rendered = format!("{error:?}");
    assert!(!rendered.contains(PRIMARY_SEED));
}

// r[verify kiln.radicle.mutation_scope.scenario.foreign]
#[test]
fn foreign_head_is_rejected_before_write() {
    let fixture = initialize_fixture();
    let foreign_head = append_foreign_head(&fixture);
    let profile = profile(&fixture, PRIMARY_PROVIDER);
    let cob_profile = cob_profile(&profile);
    let mut transport = RadicleMutationTransport::new_fixture(
        storage_root(&fixture),
        profile.clone(),
        Duration::ZERO,
    )
    .unwrap_or_else(|_| unreachable!());
    let plan = plan(&fixture, &profile, &foreign_head);
    let expected = transport.observe(&plan).unwrap_or_else(|_| unreachable!());
    let error = port_error(dispatch_mutation(
        &cob_profile,
        &expected,
        plan,
        &mut transport,
    ));
    assert_eq!(domain_code(error), "radicle_mutation_foreign_head");
}

#[cfg(unix)]
#[test]
fn failed_reference_publication_stays_unknown() {
    use std::os::unix::fs::PermissionsExt as _;

    let fixture = initialize_fixture();
    let profile = profile(&fixture, PRIMARY_PROVIDER);
    let cob_profile = cob_profile(&profile);
    let mut transport = RadicleMutationTransport::new_fixture(
        storage_root(&fixture),
        profile.clone(),
        Duration::ZERO,
    )
    .unwrap_or_else(|_| unreachable!());
    let plan = plan(&fixture, &profile, &fixture.head_id);
    let expected = transport.observe(&plan).unwrap_or_else(|_| unreachable!());
    let refs = storage_root(&fixture)
        .join(FIXTURE_RID)
        .join("refs")
        .join("namespaces")
        .join(fixture.signer.public_key())
        .join("refs")
        .join("cobs")
        .join(PATCH_TYPE_NAME);
    std::fs::set_permissions(&refs, std::fs::Permissions::from_mode(READ_EXECUTE_MODE))
        .unwrap_or_else(|_| unreachable!());
    let outcome = dispatch_mutation(&cob_profile, &expected, plan.clone(), &mut transport)
        .unwrap_or_else(|_| unreachable!());
    std::fs::set_permissions(&refs, std::fs::Permissions::from_mode(OWNER_FULL_MODE))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(outcome, CollaborationMutationOutcome::Unknown);
    assert_eq!(
        transport
            .reconcile(&plan)
            .unwrap_or_else(|_| unreachable!()),
        CollaborationMutationOutcome::Unknown
    );
}

// r[verify kiln.radicle.mutation_bounds.scenario.timeout]
// r[verify kiln.radicle.mutation_bounds.scenario.reconcile]
#[test]
fn accepted_write_times_out_to_unknown_then_reconciles() {
    let fixture = initialize_fixture();
    let mut profile = profile(&fixture, PRIMARY_PROVIDER);
    profile.deadlines.write_ms = TIMEOUT_DEADLINE_MS;
    let cob_profile = cob_profile(&profile);
    let mut transport = RadicleMutationTransport::new_fixture(
        storage_root(&fixture),
        profile.clone(),
        Duration::from_millis(TIMEOUT_DELAY_MS),
    )
    .unwrap_or_else(|_| unreachable!());
    let plan = plan(&fixture, &profile, &fixture.head_id);
    let expected = transport.observe(&plan).unwrap_or_else(|_| unreachable!());
    let outcome = dispatch_mutation(&cob_profile, &expected, plan.clone(), &mut transport)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(outcome, CollaborationMutationOutcome::Unknown);
    assert!(matches!(
        transport
            .reconcile(&plan)
            .unwrap_or_else(|_| unreachable!()),
        CollaborationMutationOutcome::Applied(_)
    ));
}
