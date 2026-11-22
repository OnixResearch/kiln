use super::*;
use crate::implementation::authority::ProtectedOperation as Op;
use crate::implementation::{
    AcceptanceObligation, AttemptLedger, DEFAULT_LEASE_MS, IMPLEMENTATION_WORK_SCHEMA,
    ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
};
use crate::types::ExternalRef;
use alloc::vec;
use alloc::vec::Vec;

const NOW: u64 = 1_000;
const ROOT_NOT_AFTER: u64 = 10_000;
const CHILD_NOT_AFTER: u64 = 5_000;
const GENERATION: u64 = 3;
const REVOCATION: u64 = 5;
const ROOT_DEPTH: u8 = 2;
const AUDIENCE: &str = "did:web:kiln.example";
const POLICY: &str = "policy:kiln-default";
const COORDINATOR: &str = "did:key:coordinator";
const WORKER: &str = "did:key:worker";
const HELPER: &str = "did:key:helper";
const SOURCE_VIEW: &str = "source-view";
const CAIRN_CONTEXT: &str = "cairn-context";
const CODING: [Op; 3] = [Op::ClaimRenew, Op::Dispatch, Op::CandidateSubmit];

fn id(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn grant_ref(byte: u8) -> [u8; AUTHORITY_DIGEST_BYTES] {
    [byte; AUTHORITY_DIGEST_BYTES]
}

fn operations(list: &[Op]) -> BTreeSet<Op> {
    list.iter().copied().collect()
}

fn routes(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|route| (*route).to_string()).collect()
}

fn lineage() -> AttemptLineage {
    AttemptLineage {
        campaign_revision: Some(id('c')),
        project: "kiln".to_string(),
        work: id('1'),
        work_version: 2,
        attempt: id('a'),
        fence: 7,
    }
}

fn root_budget() -> GrantBudget {
    GrantBudget {
        parent_budget_identity: id('b'),
        account: id('d'),
    }
}

fn disjoint_budget() -> GrantBudget {
    GrantBudget {
        parent_budget_identity: id('d'),
        account: id('e'),
    }
}

/// Owner facts the shell would read: clock, generations, revocation view and
/// the current attempt.
struct World {
    now_s: u64,
    authority_generation: u64,
    revocation_generation: u64,
    revoked: BTreeSet<[u8; AUTHORITY_DIGEST_BYTES]>,
    attempt: AttemptLineage,
}

impl World {
    fn new() -> Self {
        Self {
            now_s: NOW,
            authority_generation: GENERATION,
            revocation_generation: REVOCATION,
            revoked: BTreeSet::new(),
            attempt: lineage(),
        }
    }

    fn current(&self) -> CurrentFacts<'_> {
        CurrentFacts {
            now_s: self.now_s,
            authority_generation: self.authority_generation,
            revocation_generation: self.revocation_generation,
            revoked: &self.revoked,
            attempt: &self.attempt,
        }
    }
}

fn root_terms(operations: BTreeSet<Op>) -> CeilingTerms {
    CeilingTerms {
        audience: AUDIENCE.to_string(),
        project: "kiln".to_string(),
        work: Some(id('1')),
        operations,
        not_after_s: ROOT_NOT_AFTER,
        max_depth: ROOT_DEPTH,
        disclosure_routes: routes(&[SOURCE_VIEW, CAIRN_CONTEXT]),
    }
}

fn root_request(operations: BTreeSet<Op>) -> RootGrantRequest {
    root_request_from(root_terms(operations))
}

fn root_request_from(terms: CeilingTerms) -> RootGrantRequest {
    RootGrantRequest {
        ceiling: AuthorityCeiling::new(GENERATION, terms).unwrap_or_else(|_| unreachable!()),
        root_policy_ref: POLICY.to_string(),
        holder: COORDINATOR.to_string(),
        incarnation: None,
        budget: root_budget(),
    }
}

fn root_with(operations: BTreeSet<Op>) -> AttemptAuthorityBinding {
    root_from(root_terms(operations))
}

fn root_from(terms: CeilingTerms) -> AttemptAuthorityBinding {
    let world = World::new();
    let budget = root_budget();
    admit_root(
        root_request_from(terms),
        &RootFacts {
            current: world.current(),
            budget: Some(&budget),
        },
    )
    .ok()
    .and_then(|grant| grant.issue(grant_ref(1)).ok())
    .unwrap_or_else(|| unreachable!())
}

fn root() -> AttemptAuthorityBinding {
    root_with(Op::ALL.into_iter().collect())
}

fn child_request() -> ChildGrantRequest {
    ChildGrantRequest {
        terms: CeilingTerms {
            audience: AUDIENCE.to_string(),
            project: "kiln".to_string(),
            work: Some(id('1')),
            operations: operations(&CODING),
            not_after_s: CHILD_NOT_AFTER,
            max_depth: 1,
            disclosure_routes: routes(&[SOURCE_VIEW]),
        },
        holder: WORKER.to_string(),
        incarnation: Some(id('9')),
        budget: disjoint_budget(),
    }
}

/// One delegation attempt with every fact explicit, so each negative case
/// changes exactly one input of an otherwise admitted request.
struct Case {
    parent: AttemptAuthorityBinding,
    ancestors: Vec<AttemptAuthorityBinding>,
    request: ChildGrantRequest,
    delegator: String,
    delegate_permitted: bool,
    budget: Option<GrantBudget>,
    world: World,
}

impl Case {
    fn from_root() -> Self {
        Self {
            parent: root(),
            ancestors: Vec::new(),
            request: child_request(),
            delegator: COORDINATOR.to_string(),
            delegate_permitted: true,
            budget: Some(disjoint_budget()),
            world: World::new(),
        }
    }

    /// Delegate from the worker's coding child to a helper, sharing its account.
    fn from_child() -> Self {
        let root = root();
        let child = issue(&Self::from_root(), 2);
        let mut request = child_request();
        request.holder = HELPER.to_string();
        request.terms.operations = operations(&[Op::Dispatch]);
        request.terms.max_depth = 0;
        request.terms.not_after_s = CHILD_NOT_AFTER - 1;
        request.budget = disjoint_budget();
        Self {
            parent: child,
            ancestors: vec![root],
            request,
            delegator: WORKER.to_string(),
            delegate_permitted: true,
            budget: Some(disjoint_budget()),
            world: World::new(),
        }
    }

    fn run(&self) -> Result<AdmittedGrant, LineageError> {
        admit_child(
            &self.parent,
            self.request.clone(),
            &DelegationFacts {
                current: self.world.current(),
                delegator: &self.delegator,
                delegate_permitted: self.delegate_permitted,
                ancestors: &self.ancestors,
                budget: self.budget.as_ref(),
            },
        )
    }
}

fn issue(case: &Case, byte: u8) -> AttemptAuthorityBinding {
    case.run()
        .ok()
        .and_then(|grant| grant.issue(grant_ref(byte)).ok())
        .unwrap_or_else(|| unreachable!())
}

fn refusal(result: Result<AdmittedGrant, LineageError>) -> Option<LineageRefusal> {
    match result {
        Err(LineageError::Refused(refusal)) => Some(refusal),
        _ => None,
    }
}

#[test]
// r[verify kiln.attempt.grant_lineage.root] r[verify kiln.attempt.grant_lineage.root.scenario.refused]
fn a_root_binds_the_admitted_ceiling_to_the_current_attempt() {
    let root = root();
    assert_eq!(root.validate(), Ok(()));
    assert_eq!(root.schema, ATTEMPT_AUTHORITY_BINDING_SCHEMA);
    assert_eq!(root.lineage(), lineage());
    assert_eq!((root.parent.clone(), root.depth), (None, 0));
    assert_eq!(root.revocation_generation, REVOCATION);
    let world = World::new();
    assert_eq!(check_current(&root, &[], &world.current()), Ok(()));

    let admit = |request: RootGrantRequest, world: &World, budget: Option<&GrantBudget>| {
        admit_root(
            request,
            &RootFacts {
                current: world.current(),
                budget,
            },
        )
    };
    let budget = root_budget();
    let all = || Op::ALL.into_iter().collect::<BTreeSet<_>>();
    let mut moved = World::new();
    moved.authority_generation = GENERATION + 1;
    let mut expired = World::new();
    expired.now_s = ROOT_NOT_AFTER;
    let mut other_work = World::new();
    other_work.attempt.work = id('2');
    let mut other_project = World::new();
    other_project.attempt.project = "other".to_string();
    let mut other_budget = root_budget();
    other_budget.account = id('f');
    let cases: [(&str, LineageRefusal, &World, Option<&GrantBudget>); 6] = [
        (
            "moved generation",
            LineageRefusal::StaleGeneration,
            &moved,
            Some(&budget),
        ),
        (
            "expired ceiling",
            LineageRefusal::OutlivesParent,
            &expired,
            Some(&budget),
        ),
        (
            "ceiling for other work",
            LineageRefusal::ForeignLineage,
            &other_work,
            Some(&budget),
        ),
        (
            "ceiling for another project",
            LineageRefusal::ForeignLineage,
            &other_project,
            Some(&budget),
        ),
        (
            "no reservation",
            LineageRefusal::BudgetUnbound,
            &world,
            None,
        ),
        (
            "other reservation",
            LineageRefusal::BudgetUnbound,
            &world,
            Some(&other_budget),
        ),
    ];
    for (name, expected, world, budget) in cases {
        assert_eq!(
            refusal(admit(root_request(all()), world, budget)),
            Some(expected),
            "{name}"
        );
    }
    let mut foreign_terms = root_terms(all());
    foreign_terms.project = "other".to_string();
    foreign_terms.work = None;
    let mut project_wide = root_request(all());
    project_wide.ceiling =
        AuthorityCeiling::new(GENERATION, foreign_terms).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        refusal(admit(project_wide, &world, Some(&budget))),
        Some(LineageRefusal::ForeignLineage),
        "a project-wide ceiling from another project cannot bind this attempt"
    );
    let mut own_terms = root_terms(all());
    own_terms.work = None;
    let mut own_project = root_request(all());
    own_project.ceiling =
        AuthorityCeiling::new(GENERATION, own_terms).unwrap_or_else(|_| unreachable!());
    assert!(
        admit(own_project, &world, Some(&budget)).is_ok(),
        "a project-wide ceiling for the work's own project binds"
    );
    let mut forged = root_request(all());
    forged.ceiling.max_depth = MAX_GRANT_DEPTH;
    assert!(matches!(
        admit(forged, &world, Some(&budget)),
        Err(LineageError::Invalid(_))
    ));
}

#[test]
// r[verify kiln.attempt.grant_lineage.attenuation.scenario.permitted]
fn an_admitted_parent_derives_a_permitted_child_and_grandchild() {
    let case = Case::from_root();
    let root = case.parent.clone();
    let child = issue(&case, 2);
    assert_eq!(child.validate(), Ok(()));
    assert_eq!(child.parent.as_ref(), Some(&root.identity));
    assert_eq!(child.depth, 1);
    assert_eq!(child.lineage(), root.lineage());
    assert_eq!(child.root_policy_ref, root.root_policy_ref);
    assert_eq!(child.ceiling.generation, root.ceiling.generation);
    assert_eq!(child.holder, WORKER);
    assert!(root.ceiling.covers(&child.ceiling));
    assert_eq!(
        check_current(&child, core::slice::from_ref(&root), &case.world.current()),
        Ok(())
    );

    let mut shared = Case::from_root();
    shared.request.budget = root_budget();
    shared.budget = Some(root_budget());
    assert!(shared.run().is_ok(), "shared-parent charging admits");

    let grand_case = Case::from_child();
    let grandchild = issue(&grand_case, 3);
    assert_eq!(grandchild.depth, 2);
    assert_eq!(grandchild.ceiling.max_depth, 0);
    assert_eq!(
        check_current(
            &grandchild,
            &[root, grand_case.parent.clone()],
            &grand_case.world.current()
        ),
        Ok(())
    );
}

type Change = fn(&mut Case);
/// Name, base case, one-input change and the refusal it must produce.
type RefusalCase = (&'static str, fn() -> Case, Change, LineageRefusal);

#[test]
// r[verify kiln.attempt.grant_lineage.attenuation] r[verify kiln.attempt.grant_lineage.attenuation.scenario.refused] r[verify kiln.attempt.grant_lineage.foreign.scenario.refused] r[verify kiln.attempt.grant_lineage.budget.scenario.refused]
fn every_lineage_refusal_has_a_negative_fixture() {
    let from_root = Case::from_root;
    let from_child = Case::from_child;
    let cases: [RefusalCase; 38] = [
        (
            "other project",
            from_root,
            |c| c.request.terms.project = "other".to_string(),
            LineageRefusal::WidenResource,
        ),
        (
            "project-wide work",
            from_root,
            |c| c.request.terms.work = None,
            LineageRefusal::WidenResource,
        ),
        (
            "new disclosure route",
            from_root,
            |c| {
                c.request
                    .terms
                    .disclosure_routes
                    .insert("secret-view".to_string());
            },
            LineageRefusal::WidenResource,
        ),
        (
            "extra operation",
            from_child,
            |c| {
                c.request.terms.operations.insert(Op::AttemptCancel);
            },
            LineageRefusal::WidenOperation,
        ),
        (
            "other audience",
            from_root,
            |c| c.request.terms.audience = "did:web:other".to_string(),
            LineageRefusal::WidenAudience,
        ),
        (
            "outlives parent",
            from_root,
            |c| c.request.terms.not_after_s = ROOT_NOT_AFTER + 1,
            LineageRefusal::OutlivesParent,
        ),
        (
            "expired parent",
            from_child,
            |c| c.world.now_s = CHILD_NOT_AFTER,
            LineageRefusal::OutlivesParent,
        ),
        (
            "expired root",
            from_child,
            |c| c.world.now_s = ROOT_NOT_AFTER,
            LineageRefusal::OutlivesParent,
        ),
        (
            "equal depth",
            from_root,
            |c| c.request.terms.max_depth = ROOT_DEPTH,
            LineageRefusal::DepthExceeded,
        ),
        (
            "chain over bound",
            from_root,
            |c| {
                c.ancestors = vec![root(); MAX_GRANT_LINEAGE];
            },
            LineageRefusal::DepthExceeded,
        ),
        (
            "no delegate permission",
            from_root,
            |c| c.delegate_permitted = false,
            LineageRefusal::DelegationNotPermitted,
        ),
        (
            "leaf parent",
            from_child,
            |c| {
                let leaf = issue(c, 3);
                c.ancestors.push(c.parent.clone());
                c.parent = leaf;
                c.delegator = HELPER.to_string();
            },
            LineageRefusal::DelegationNotPermitted,
        ),
        (
            "coding to verification",
            from_child,
            |c| {
                c.request.terms.operations = operations(&[Op::VerificationAdmit]);
            },
            LineageRefusal::RoleEscalation,
        ),
        (
            "coding to publication",
            from_child,
            |c| {
                c.request.terms.operations = operations(&[Op::PublicationPublish]);
            },
            LineageRefusal::RoleEscalation,
        ),
        (
            "coding to lifecycle",
            from_child,
            |c| {
                c.request.terms.operations = operations(&[Op::LifecycleComplete]);
            },
            LineageRefusal::RoleEscalation,
        ),
        (
            "mixed roles",
            from_root,
            |c| {
                c.request.terms.operations.insert(Op::IntegrationLand);
            },
            LineageRefusal::RoleEscalation,
        ),
        (
            "verification to publication",
            from_root,
            |c| {
                c.request.terms.operations = operations(&[Op::VerificationAdmit]);
                let verifier = issue(c, 4);
                c.ancestors = vec![c.parent.clone()];
                c.parent = verifier;
                c.delegator = WORKER.to_string();
                c.request.terms.operations = operations(&[Op::PublicationPublish]);
                c.request.terms.max_depth = 0;
            },
            LineageRefusal::RoleEscalation,
        ),
        (
            "other work",
            from_root,
            |c| c.world.attempt.work = id('2'),
            LineageRefusal::ForeignLineage,
        ),
        (
            "other version",
            from_root,
            |c| c.world.attempt.work_version = 3,
            LineageRefusal::ForeignLineage,
        ),
        (
            "other attempt",
            from_root,
            |c| c.world.attempt.attempt = id('f'),
            LineageRefusal::ForeignLineage,
        ),
        (
            "other campaign",
            from_root,
            |c| c.world.attempt.campaign_revision = None,
            LineageRefusal::ForeignLineage,
        ),
        (
            "holder is not the delegator",
            from_root,
            |c| c.delegator = WORKER.to_string(),
            LineageRefusal::ForeignLineage,
        ),
        (
            "copied parent",
            from_root,
            |c| {
                c.parent.ceiling.operations.remove(&Op::LifecycleComplete);
            },
            LineageRefusal::ForeignLineage,
        ),
        (
            "missing ancestors",
            from_child,
            |c| c.ancestors.clear(),
            LineageRefusal::ForeignLineage,
        ),
        (
            "child for other work",
            from_root,
            |c| c.request.terms.work = Some(id('2')),
            LineageRefusal::ForeignLineage,
        ),
        (
            "no reservation",
            from_root,
            |c| c.budget = None,
            LineageRefusal::BudgetUnbound,
        ),
        (
            "reservation differs from owner",
            from_root,
            |c| c.request.budget.account = id('f'),
            LineageRefusal::BudgetUnbound,
        ),
        (
            "unrelated account",
            from_root,
            |c| {
                c.request.budget.parent_budget_identity = id('f');
                c.budget = Some(c.request.budget.clone());
            },
            LineageRefusal::BudgetUnbound,
        ),
        (
            "moved authority generation",
            from_root,
            |c| c.world.authority_generation = GENERATION + 1,
            LineageRefusal::StaleGeneration,
        ),
        (
            "older revocation view",
            from_root,
            |c| c.world.revocation_generation = REVOCATION - 1,
            LineageRefusal::StaleGeneration,
        ),
        (
            "revoked parent",
            from_root,
            |c| {
                c.world.revoked.insert(grant_ref(1));
            },
            LineageRefusal::StaleGeneration,
        ),
        (
            "revoked root under child",
            from_child,
            |c| {
                c.world.revoked.insert(grant_ref(1));
            },
            LineageRefusal::StaleGeneration,
        ),
        (
            "predecessor fence",
            from_root,
            |c| c.world.attempt.fence = 8,
            LineageRefusal::StaleGeneration,
        ),
        (
            "delayed predecessor",
            from_child,
            |c| c.world.attempt.fence = 6,
            LineageRefusal::StaleGeneration,
        ),
        (
            "successor attempt with a new fence",
            from_root,
            |c| {
                c.world.attempt.attempt = id('f');
                c.world.attempt.fence = 8;
            },
            LineageRefusal::StaleGeneration,
        ),
        (
            "successor attempt under a child",
            from_child,
            |c| {
                c.world.attempt.attempt = id('f');
                c.world.attempt.fence = 8;
            },
            LineageRefusal::StaleGeneration,
        ),
        (
            "other work with a new fence",
            from_root,
            |c| {
                c.world.attempt.work = id('2');
                c.world.attempt.attempt = id('f');
                c.world.attempt.fence = 8;
            },
            LineageRefusal::ForeignLineage,
        ),
        (
            "attempt in another project",
            from_child,
            |c| c.world.attempt.project = "other".to_string(),
            LineageRefusal::ForeignLineage,
        ),
    ];
    let mut covered = BTreeSet::new();
    for (name, base, change, expected) in cases {
        let admitted = base();
        assert!(admitted.run().is_ok(), "{name}: base case admits");
        let mut case = base();
        change(&mut case);
        assert_eq!(refusal(case.run()), Some(expected), "{name}");
        covered.insert(expected.as_str());
    }
    let all: BTreeSet<&str> = LineageRefusal::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(covered, all, "every refusal variant has a negative fixture");

    let mut malformed = Case::from_root();
    malformed.request.holder = " padded".to_string();
    assert!(matches!(malformed.run(), Err(LineageError::Invalid(_))));
    let mut empty = Case::from_root();
    empty.request.terms.operations.clear();
    assert!(matches!(empty.run(), Err(LineageError::Invalid(_))));
    let mut stale_request = Case::from_root();
    stale_request.request.terms.not_after_s = NOW;
    assert!(matches!(stale_request.run(), Err(LineageError::Invalid(_))));
}

#[test]
// r[verify kiln.attempt.grant_lineage.currentness] r[verify kiln.attempt.grant_lineage.currentness.scenario.propagate]
fn revocation_and_expiry_propagate_to_every_descendant() {
    let grand_case = Case::from_child();
    let root = grand_case
        .ancestors
        .first()
        .cloned()
        .unwrap_or_else(|| unreachable!());
    let child = grand_case.parent.clone();
    let grandchild = issue(&grand_case, 3);
    let chain = [root.clone(), child.clone()];
    let check = |world: &World| check_current(&grandchild, &chain, &world.current());
    assert_eq!(check(&World::new()), Ok(()));

    for revoked in [grant_ref(1), grant_ref(2), grant_ref(3)] {
        let mut world = World::new();
        world.revoked.insert(revoked);
        assert_eq!(
            check(&world),
            Err(LineageRefusal::StaleGeneration),
            "{revoked:?}"
        );
    }
    let mut unrelated = World::new();
    unrelated.revoked.insert(grant_ref(9));
    unrelated.revocation_generation = REVOCATION + 1;
    assert_eq!(
        check(&unrelated),
        Ok(()),
        "a newer view without this lineage keeps it"
    );

    let mut grandchild_expired = World::new();
    grandchild_expired.now_s = grandchild.ceiling.not_after_s;
    assert_eq!(
        check(&grandchild_expired),
        Err(LineageRefusal::OutlivesParent)
    );
    let mut root_expired = World::new();
    root_expired.now_s = ROOT_NOT_AFTER;
    assert_eq!(
        check_current(
            &child,
            core::slice::from_ref(&root),
            &root_expired.current()
        ),
        Err(LineageRefusal::OutlivesParent)
    );
    assert!(grandchild.ceiling.not_after_s <= child.ceiling.not_after_s);
    assert!(child.ceiling.not_after_s <= root.ceiling.not_after_s);

    let mut replaced = World::new();
    replaced.attempt.fence = 8;
    assert_eq!(check(&replaced), Err(LineageRefusal::StaleGeneration));
    assert_eq!(
        check_current(
            &grandchild,
            core::slice::from_ref(&child),
            &World::new().current()
        ),
        Err(LineageRefusal::ForeignLineage),
        "a chain without its root does not prove lineage"
    );
    assert_eq!(
        check_current(&grandchild, &[child, root], &World::new().current()),
        Err(LineageRefusal::ForeignLineage),
        "ancestors must be root first"
    );
}

#[test]
// r[verify kiln.attempt.grant_lineage.identity] r[verify kiln.attempt.grant_lineage.identity.scenario.deterministic]
fn identities_are_deterministic_blake3_over_every_field() {
    let case = Case::from_root();
    let first = case.run().unwrap_or_else(|_| unreachable!());
    let again = case.run().unwrap_or_else(|_| unreachable!());
    assert_eq!(first, again);
    assert_eq!(first.validate(), Ok(()));
    let binding = first
        .clone()
        .issue(grant_ref(2))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        binding,
        again.issue(grant_ref(2)).unwrap_or_else(|_| unreachable!())
    );
    assert_eq!(binding.identity.as_str().len(), 64);
    assert_ne!(binding.identity, first.intent);
    assert_eq!(
        first.binding_identity(&grant_ref(2)),
        Ok(binding.identity.clone())
    );
    assert!(
        first.clone().issue(grant_ref(0)).is_err(),
        "a proof reference is required"
    );

    let other_ref = first
        .clone()
        .issue(grant_ref(3))
        .unwrap_or_else(|_| unreachable!());
    assert_ne!(other_ref.identity, binding.identity);
    assert_eq!(other_ref.lineage(), binding.lineage());

    type Vary = fn(&mut Case);
    let variations: [(&str, Vary); 6] = [
        ("holder", |c| c.request.holder = HELPER.to_string()),
        ("incarnation", |c| c.request.incarnation = Some(id('8'))),
        ("no incarnation", |c| c.request.incarnation = None),
        ("lifetime", |c| c.request.terms.not_after_s -= 1),
        ("revocation generation", |c| {
            c.world.revocation_generation += 1
        }),
        ("shared budget", |c| {
            c.request.budget = root_budget();
            c.budget = Some(root_budget());
        }),
    ];
    let mut intents = BTreeSet::from([first.intent.clone()]);
    for (name, vary) in variations {
        let mut varied = Case::from_root();
        vary(&mut varied);
        let grant = varied.run().unwrap_or_else(|_| unreachable!());
        assert!(intents.insert(grant.intent), "{name} changes the intent");
    }

    let mut tampered = binding.clone();
    tampered.holder = HELPER.to_string();
    assert!(tampered.validate().is_err());
    let mut tampered_ceiling = binding.clone();
    tampered_ceiling.ceiling.not_after_s += 1;
    assert!(tampered_ceiling.validate().is_err());
    let mut tampered_intent = first;
    tampered_intent.depth = 2;
    assert!(tampered_intent.validate().is_err());

    let json = serde_json::to_value(&binding).unwrap_or_else(|_| unreachable!());
    let restored: AttemptAuthorityBinding =
        serde_json::from_value(json.clone()).unwrap_or_else(|_| unreachable!());
    assert_eq!(restored, binding);
    assert_eq!(restored.validate(), Ok(()));
    let keys: BTreeSet<&str> = json
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    assert_eq!(
        keys,
        BTreeSet::from([
            "schema",
            "ceiling",
            "root_policy_ref",
            "grant_ref",
            "holder",
            "incarnation",
            "campaign_revision",
            "work",
            "work_version",
            "attempt",
            "fence",
            "budget",
            "parent",
            "depth",
            "revocation_generation",
            "identity",
        ]),
        "the binding carries proof references, never token bytes"
    );
    let mut extra = json;
    if let Some(object) = extra.as_object_mut() {
        object.insert("token".to_string(), serde_json::Value::from("eyJ..."));
    }
    assert!(serde_json::from_value::<AttemptAuthorityBinding>(extra).is_err());
}

/// Every operation subset as a parent ceiling against every subset as a child:
/// a child is admitted exactly when it holds one role and no operation outside
/// its parent.
#[test]
// r[verify kiln.attempt.grant_lineage.attenuation.scenario.property]
fn no_admitted_child_covers_an_operation_its_parent_lacks() {
    let subsets: Vec<BTreeSet<Op>> = (1_u16..(1 << Op::ALL.len()))
        .map(|mask| {
            Op::ALL
                .iter()
                .enumerate()
                .filter(|(bit, _)| mask & (1 << bit) != 0)
                .map(|(_, operation)| *operation)
                .collect()
        })
        .collect();
    let mut admitted = 0_usize;
    for parent_operations in &subsets {
        let mut case = Case::from_root();
        case.parent = root_with(parent_operations.clone());
        for child_operations in &subsets {
            case.request.terms.operations = child_operations.clone();
            let roles: BTreeSet<GrantRole> = child_operations
                .iter()
                .map(|op| GrantRole::of(*op))
                .collect();
            let permitted = roles.len() == 1 && child_operations.is_subset(parent_operations);
            match case.run() {
                Ok(grant) => {
                    assert!(permitted, "{parent_operations:?} -> {child_operations:?}");
                    assert!(case.parent.ceiling.covers(&grant.ceiling));
                    admitted += 1;
                }
                Err(LineageError::Refused(
                    LineageRefusal::WidenOperation | LineageRefusal::RoleEscalation,
                )) => assert!(!permitted, "{parent_operations:?} -> {child_operations:?}"),
                Err(other) => unreachable!("{other:?}"),
            }
        }
    }
    assert!(admitted > 0);
}

/// Every combination of lifetime, depth, route, work, project and audience
/// under a work-scoped and a project-wide parent. An explicit oracle, written
/// independently of `covers`, decides which children admit; every admitted
/// child is also covered by its parent.
#[test]
// r[verify kiln.attempt.grant_lineage.attenuation.scenario.property]
fn no_admitted_child_exceeds_its_parent_in_any_dimension() {
    let lifetimes = [CHILD_NOT_AFTER, ROOT_NOT_AFTER, ROOT_NOT_AFTER + 1];
    let route_sets = [
        routes(&[]),
        routes(&[SOURCE_VIEW]),
        routes(&[SOURCE_VIEW, CAIRN_CONTEXT]),
        routes(&[SOURCE_VIEW, "secret-view"]),
    ];
    let works = [Some(id('1')), None];
    let projects = ["kiln", "other"];
    let audiences = [AUDIENCE, "did:web:other"];
    let mut project_wide_terms = root_terms(Op::ALL.into_iter().collect());
    project_wide_terms.work = None;
    let parents = [(root(), false), (root_from(project_wide_terms), true)];
    for (parent, project_wide) in parents {
        let mut admitted = 0_usize;
        for not_after_s in lifetimes {
            for max_depth in 0..=MAX_GRANT_DEPTH {
                for (route_index, disclosure_routes) in route_sets.iter().enumerate() {
                    for work in &works {
                        for project in projects {
                            for audience in audiences {
                                let mut case = Case::from_root();
                                case.parent = parent.clone();
                                case.request.terms = CeilingTerms {
                                    audience: audience.to_string(),
                                    project: project.to_string(),
                                    work: work.clone(),
                                    operations: operations(&CODING),
                                    not_after_s,
                                    max_depth,
                                    disclosure_routes: disclosure_routes.clone(),
                                };
                                // Oracle: the parent holds audience AUDIENCE,
                                // project "kiln", routes {source-view,
                                // cairn-context} (route sets 0..=2),
                                // lifetime ROOT_NOT_AFTER and depth ROOT_DEPTH;
                                // a work-scoped parent also holds work '1'.
                                let expected = audience == AUDIENCE
                                    && project == "kiln"
                                    && (project_wide || work.is_some())
                                    && route_index <= 2
                                    && not_after_s <= ROOT_NOT_AFTER
                                    && max_depth < ROOT_DEPTH;
                                let result = case.run();
                                assert_eq!(
                                    result.is_ok(),
                                    expected,
                                    "{project_wide} {audience} {project} {work:?} {route_index} {not_after_s} {max_depth}"
                                );
                                if let Ok(grant) = result {
                                    assert!(case.parent.ceiling.covers(&grant.ceiling));
                                    admitted += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        // Two lifetimes, two depths and three route sets admit; a project-wide
        // parent also admits the project-wide child.
        let works_admitted = if project_wide { 2 } else { 1 };
        assert_eq!(admitted, 2 * 2 * 3 * works_admitted);
    }
}

/// Rebuild a binding's ceiling and binding identities after an edit, so the
/// link recomputes and only the lineage rules can refuse it.
fn reseal(mut binding: AttemptAuthorityBinding) -> AttemptAuthorityBinding {
    binding.ceiling.identity = ContentId::from_blake3_hex(binding.ceiling.view().digest_hex())
        .unwrap_or_else(|_| unreachable!());
    binding.identity =
        ContentId::from_blake3_hex(binding.fields().binding_digest_hex(&binding.grant_ref))
            .unwrap_or_else(|_| unreachable!());
    binding
}

#[test]
// r[verify kiln.attempt.grant_lineage.foreign] r[verify kiln.attempt.grant_lineage.foreign.scenario.refused]
fn check_current_refuses_every_recomputing_link_that_breaks_lineage() {
    let case = Case::from_root();
    let root = case.parent.clone();
    let child = issue(&case, 2);
    let world = World::new();
    let ancestors = core::slice::from_ref(&root);
    assert_eq!(check_current(&child, ancestors, &world.current()), Ok(()));

    type Edit = fn(&mut AttemptAuthorityBinding);
    let edits: [(&str, Edit); 7] = [
        ("other parent identity", |b| b.parent = Some(id('9'))),
        ("wider lifetime", |b| {
            b.ceiling.not_after_s = ROOT_NOT_AFTER + 1
        }),
        ("wider route", |b| {
            b.ceiling
                .disclosure_routes
                .insert("secret-view".to_string());
        }),
        ("depth 2 under the root", |b| b.depth = 2),
        ("other root policy", |b| {
            b.root_policy_ref = "policy:other".to_string()
        }),
        ("mixed roles", |b| {
            b.ceiling.operations.insert(Op::VerificationAdmit);
        }),
        ("unrelated budget", |b| {
            b.budget.parent_budget_identity = id('f')
        }),
    ];
    for (name, edit) in edits {
        let mut forged = child.clone();
        edit(&mut forged);
        let forged = reseal(forged);
        assert_eq!(forged.validate(), Ok(()), "{name}: the link recomputes");
        assert_eq!(
            check_current(&forged, ancestors, &world.current()),
            Err(LineageRefusal::ForeignLineage),
            "{name}"
        );
    }

    // A sibling root R2 differs from R1 only in its proof reference, so every
    // other link rule passes; only the parent identity tells them apart.
    let sibling_root = admit_root(
        root_request(Op::ALL.into_iter().collect()),
        &RootFacts {
            current: world.current(),
            budget: Some(&root_budget()),
        },
    )
    .ok()
    .and_then(|grant| grant.issue(grant_ref(9)).ok())
    .unwrap_or_else(|| unreachable!());
    assert_ne!(sibling_root.identity, root.identity);
    assert_eq!(check_current(&sibling_root, &[], &world.current()), Ok(()));
    assert_eq!(
        check_current(
            &child,
            core::slice::from_ref(&sibling_root),
            &world.current()
        ),
        Err(LineageRefusal::ForeignLineage),
        "a child of R1 presented under R2 is foreign"
    );
}

#[test]
// r[verify kiln.attempt.grant_lineage.issuance]
fn an_intent_binds_only_its_exact_attempt_lineage() {
    let grant = Case::from_root().run().unwrap_or_else(|_| unreachable!());
    assert!(grant.binds(&lineage()));
    type Flip = fn(&mut AttemptLineage);
    let flips: [(&str, Flip); 6] = [
        ("campaign revision", |l| l.campaign_revision = Some(id('d'))),
        ("project", |l| l.project = "other".to_string()),
        ("work", |l| l.work = id('2')),
        ("work version", |l| l.work_version += 1),
        ("attempt", |l| l.attempt = id('f')),
        ("fence", |l| l.fence += 1),
    ];
    for (name, flip) in flips {
        let mut other = lineage();
        flip(&mut other);
        assert!(!grant.binds(&other), "{name}");
    }
}

fn work() -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: 1,
        actor: "operator".to_string(),
        repository: "onix/kiln".to_string(),
        project: "kiln".to_string(),
        base: None,
        title: "Grant lineage work".to_string(),
        requested_outcome: "implement".to_string(),
        acceptance: vec![AcceptanceObligation {
            id: "accepted".to_string(),
            statement: "verified".to_string(),
        }],
        dependencies: Vec::new(),
        permitted_paths: vec!["crates".to_string()],
        permitted_effects: BTreeSet::from(["write_source".to_string()]),
        verification_profile: id('9'),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#70")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#70".to_string(),
            version: id('1'),
        }],
    }
}

fn attempt_root(ledger: &AttemptLedger, attempt: &ContentId, proof: u8) -> AttemptAuthorityBinding {
    let claimed = ledger.get(attempt).unwrap_or_else(|| unreachable!());
    let mut world = World::new();
    world.attempt = AttemptLineage {
        campaign_revision: None,
        project: "kiln".to_string(),
        work: claimed.work_identity.clone(),
        work_version: claimed.work_revision,
        attempt: claimed.identity.clone(),
        fence: claimed.fence,
    };
    let mut terms = root_terms(operations(&CODING));
    terms.work = Some(claimed.work_identity.clone());
    let budget = root_budget();
    admit_root(
        RootGrantRequest {
            ceiling: AuthorityCeiling::new(GENERATION, terms).unwrap_or_else(|_| unreachable!()),
            root_policy_ref: POLICY.to_string(),
            holder: COORDINATOR.to_string(),
            incarnation: None,
            budget: budget.clone(),
        },
        &RootFacts {
            current: world.current(),
            budget: Some(&budget),
        },
    )
    .ok()
    .and_then(|grant| grant.issue(grant_ref(proof)).ok())
    .unwrap_or_else(|| unreachable!())
}

#[test]
// r[verify kiln.attempt.grant_lineage.attempt] r[verify kiln.attempt.grant_lineage.attempt.scenario.bound]
fn the_attempt_retains_one_exact_binding_and_round_trips_without_one() {
    let work = work();
    let mut ledger = AttemptLedger::default();
    let attempt = ledger
        .claim(&work, 0, DEFAULT_LEASE_MS)
        .unwrap_or_else(|_| unreachable!());
    let before = serde_json::to_string(&ledger).unwrap_or_else(|_| unreachable!());
    assert!(
        !before.contains("\"authority\""),
        "an unbound attempt serializes unchanged"
    );
    let reloaded: AttemptLedger = serde_json::from_str(&before).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        serde_json::to_string(&reloaded).unwrap_or_else(|_| unreachable!()),
        before
    );

    let code = |result: CoreResult<()>| result.err().map(|diagnostic| diagnostic.code);
    let at = 1;
    let binding = attempt_root(&ledger, &attempt.identity, 1);
    let alternative = attempt_root(&ledger, &attempt.identity, 7);
    let mut tampered = binding.clone();
    tampered.fence += 1;
    assert_eq!(
        code(ledger.bind_authority(&attempt.identity, tampered, at)),
        Some("authority_binding_identity".to_string()),
        "a binding that does not recompute is refused"
    );
    assert_eq!(
        code(ledger.bind_authority(&attempt.identity, binding.clone(), at)),
        None
    );
    assert_eq!(ledger.authority(&attempt.identity), Some(&binding));
    assert_eq!(
        code(ledger.bind_authority(&attempt.identity, binding.clone(), at)),
        None,
        "the same binding is idempotent"
    );
    assert_eq!(
        code(ledger.bind_authority(&attempt.identity, alternative, at)),
        Some("implementation_authority_rebind".to_string()),
        "an alternative proof cannot replace the bound grant"
    );

    let restored: AttemptLedger =
        serde_json::from_str(&serde_json::to_string(&ledger).unwrap_or_else(|_| unreachable!()))
            .unwrap_or_else(|_| unreachable!());
    assert_eq!(restored.authority(&attempt.identity), Some(&binding));

    let mut other_work = work.clone();
    other_work.title = "Other grant lineage work".to_string();
    if let Some(alias) = other_work.aliases.first_mut() {
        alias.source =
            ExternalRef::new("github", "issue", "onix/kiln#71").unwrap_or_else(|_| unreachable!());
        alias.native_id = "onix/kiln#71".to_string();
    }
    let other = ledger
        .claim(&other_work, 0, DEFAULT_LEASE_MS)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        code(ledger.bind_authority(&other.identity, binding, at)),
        Some("implementation_authority_lineage".to_string()),
        "a binding for another attempt cannot bind this one"
    );

    let other_binding = attempt_root(&ledger, &other.identity, 2);
    assert_eq!(
        code(ledger.bind_authority(&other.identity, other_binding.clone(), DEFAULT_LEASE_MS)),
        Some("implementation_lease_expired".to_string()),
        "a lease-expired attempt binds nothing"
    );
    ledger
        .cancel(&other.identity, other.fence, 1, "operator")
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        code(ledger.bind_authority(&other.identity, other_binding, at)),
        Some("implementation_authority_inactive".to_string())
    );

    // A1 is still `Claimed` after its lease lapsed and A2 replaced it; even a
    // clock inside A1's old lease cannot bind authority to it.
    let short_lease = 10;
    let mut replaced = AttemptLedger::default();
    let first = replaced
        .claim(&work, 0, short_lease)
        .unwrap_or_else(|_| unreachable!());
    let first_binding = attempt_root(&replaced, &first.identity, 3);
    let second = replaced
        .claim(&work, short_lease + 1, DEFAULT_LEASE_MS)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        replaced.get(&first.identity).map(|a| a.phase),
        Some(crate::implementation::AttemptPhase::Claimed)
    );
    assert_ne!(first.identity, second.identity);
    assert_eq!(
        code(replaced.bind_authority(&first.identity, first_binding, at)),
        Some("implementation_fence_stale".to_string()),
        "a replaced attempt binds nothing"
    );
}
