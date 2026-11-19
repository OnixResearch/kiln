use kiln_core::{
    fair_admission::AdmissionPolicy,
    implementation::*,
    inventory::*,
    operations::FairnessStrategy,
    types::{Alias, ContentId, ExternalRef},
};
use kiln_implementer::{
    Implementer, admission::AdmissionCommand, observation::ImplementationObservation,
};
use kiln_provider_api::inventory::{InventoryReadFailure, InventoryWorkPort};
use std::collections::{BTreeMap, BTreeSet};
fn id(value: &str) -> ContentId {
    ContentId::from_blake3_hex(blake3::hash(value.as_bytes()).to_hex().to_string())
        .unwrap_or_else(|_| unreachable!())
}
fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        version: 1,
        accounting_basis: id("basis"),
        max_items: 8,
        max_bytes: 100_000,
        max_wait_ms: 1000,
        max_observation_age_ms: 100,
        max_retry_after_ms: 100,
        max_children: 4,
        max_active: 2,
        strategy: FairnessStrategy::DominantShare,
        shares: BTreeMap::from([(Alias::new("tenant").unwrap_or_else(|_| unreachable!()), 1)]),
        group_limits: vec![],
    }
}
fn work() -> ImplementationWork {
    ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.into(),
        revision: 1,
        actor: "operator".into(),
        repository: "onix/kiln".into(),
        project: "kiln".into(),
        base: None,
        title: "observe exact work".into(),
        requested_outcome: "implement".into(),
        acceptance: vec![AcceptanceObligation {
            id: "accept".into(),
            statement: "independent acceptance".into(),
        }],
        dependencies: vec![],
        permitted_paths: vec!["src".into()],
        permitted_effects: BTreeSet::from(["write_source".into()]),
        verification_profile: id("verification"),
        budget: WorkBudget {
            attempts: 2,
            minutes_per_attempt: 60,
        },
        terminal_policy: TerminalPolicy::VerifiedPullRequest,
        priority: 1,
        aliases: vec![SourceRef {
            kind: WorkSourceKind::GitHubIssue,
            source: ExternalRef::new("github", "issue", "onix/kiln#45")
                .unwrap_or_else(|_| unreachable!()),
            native_id: "onix/kiln#45".into(),
            version: id("source"),
        }],
    }
}
fn needs(work: &ImplementationWork, policy: &ContentId) -> WorkNeeds {
    WorkNeeds {
        work_version: work
            .identity()
            .unwrap_or_else(|_| unreachable!())
            .as_str()
            .into(),
        task_family: "feature".into(),
        output_contract: "candidate-v1".into(),
        definition: FactRef {
            source: "lattice".into(),
            id: "omp".into(),
        },
        profile_revision: "pinned-v1".into(),
        policy_ref: policy.as_str().into(),
        harness: "omp-jsonl".into(),
        tools: vec!["edit".into()],
        sandbox: "bubblewrap".into(),
        execution_backend: "cpu-linux".into(),
        model_backend: "remote-api".into(),
        context_tokens: 1024,
        execution_resources: vec![ResourceRequirement {
            unit: "cpu".into(),
            amount: 1,
        }],
        modalities: vec!["text".into()],
        model_artifact: Some("model-v1".into()),
        qualification_subject: "candidate".into(),
        qualification_corpus: "fixtures".into(),
        qualification_protocol: "v1".into(),
    }
}
fn store(path: &std::path::Path) -> Result<Implementer, String> {
    Implementer::open_with_local_operator(
        path,
        Enrollment {
            repositories: BTreeSet::from(["onix/kiln".into()]),
            actors: BTreeSet::from(["operator".into()]),
        },
        "test-operator",
    )
}
#[test]
fn current_missing_stale_policy_and_reopen_are_distinct() -> Result<(), Box<dyn std::error::Error>>
{
    let root = tempfile::tempdir()?;
    let path = root.path().join("work.sqlite");
    let work = work();
    let identity = work
        .identity()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let mut writer = store(&path)?;
    writer.admit(&work, None)?;
    let policy = policy();
    let policy_id = policy
        .identity()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    writer.admission_apply(
        AdmissionCommand::Configure {
            policy: policy.clone(),
        },
        0,
    )?;
    let reader = ImplementationObservation::open(&path).map_err(|e| format!("{e:?}"))?;
    assert_eq!(reader.current_requirements(&identity), Ok(None));
    assert_eq!(
        reader.current_requirements(&id("unknown")),
        Err(InventoryReadFailure::StaleWork)
    );
    let attachment = WorkNeedsAttachment::new(
        identity.clone(),
        work.revision,
        policy_id.clone(),
        needs(&work, &policy_id),
    )?;
    writer.attach_work_needs(&attachment)?;
    drop(reader);
    drop(writer);
    let reader = ImplementationObservation::open(&path).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        reader.current_requirements(&identity),
        Ok(Some(attachment.clone()))
    );
    let mut writer = store(&path)?;
    let mut changed = policy.clone();
    changed.max_wait_ms += 1;
    let policy_error = writer
        .admission_apply(AdmissionCommand::Configure { policy: changed }, 1)
        .err()
        .ok_or("immutable queue policy was unexpectedly replaced")?;
    assert!(policy_error.contains("admission_policy_frozen"));
    assert_eq!(writer.admission_snapshot()?.policy.as_ref(), Some(&policy));
    assert_eq!(reader.current_requirements(&identity), Ok(Some(attachment)));

    // A sealed attachment copied from a different immutable policy must not
    // become current here. Production admission refuses it; the direct SQL
    // write below constructs only the stale/replayed-store negative fixture.
    let other_policy = id("another-ledger-policy");
    let stale = WorkNeedsAttachment::new(
        identity.clone(),
        work.revision,
        other_policy.clone(),
        needs(&work, &other_policy),
    )?;
    assert!(writer.attach_work_needs(&stale).is_err());
    let connection = rusqlite::Connection::open(&path)?;
    connection.execute(
        "UPDATE implementation_state SET bytes=?2 WHERE kind=?1",
        rusqlite::params![
            format!("needs:{}:{}", identity.as_str(), policy_id.as_str()),
            serde_json::to_vec(&Some(stale))?,
        ],
    )?;
    assert_eq!(
        reader.current_requirements(&identity),
        Err(InventoryReadFailure::StaleWork)
    );
    assert!(writer.work_needs(&identity, &policy_id).is_err());
    let mut newer = work;
    newer.revision += 1;
    newer.title.push_str(" changed");
    writer.admit(&newer, None)?;
    assert_eq!(
        reader.current_requirements(&identity),
        Err(InventoryReadFailure::StaleWork)
    );
    Ok(())
}
#[test]
fn oversized_and_forged_records_do_not_become_current_requirements()
-> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("work.sqlite");
    let work = work();
    let identity = work
        .identity()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    let mut writer = store(&path)?;
    writer.admit(&work, None)?;
    let policy = policy();
    let policy_id = policy
        .identity()
        .map_err(|e| format!("{}: {}", e.code, e.message))?;
    writer.admission_apply(AdmissionCommand::Configure { policy }, 0)?;
    let attachment = WorkNeedsAttachment::new(
        identity.clone(),
        1,
        policy_id.clone(),
        needs(&work, &policy_id),
    )?;
    writer.attach_work_needs(&attachment)?;
    drop(writer);
    let connection = rusqlite::Connection::open(&path)?;
    let key = format!("needs:{}:{}", identity.as_str(), policy_id.as_str());
    let mut forged = attachment;
    forged.needs.profile_revision = "substituted".into();
    connection.execute(
        "UPDATE implementation_state SET bytes=?2 WHERE kind=?1",
        rusqlite::params![key, serde_json::to_vec(&Some(forged))?],
    )?;
    let reader = ImplementationObservation::open(&path).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        reader.current_requirements(&identity),
        Err(InventoryReadFailure::StaleWork)
    );
    connection.execute(
        "UPDATE implementation_state SET bytes=zeroblob(16777217) WHERE kind='work'",
        [],
    )?;
    assert_eq!(
        reader.current_requirements(&identity),
        Err(InventoryReadFailure::MalformedObservation)
    );
    Ok(())
}
