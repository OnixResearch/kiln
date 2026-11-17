//! One bounded continuation step for a supervisor, and wake hints for owners.
//!
//! Both commands reread canonical campaign state. A wake hint only asks the
//! next step to reread; it never grants an effect, a resource or completion.

use std::path::{Path, PathBuf};

use kiln_core::implementation::{
    campaign::MAX_ID_BYTES,
    continuation::{ContinuationPolicy, WakeHint, WakeKind},
};
use kiln_implementer::campaign::{CampaignObservation, CampaignStore, continue_bounded};

use super::{open_native, show, text, wall_clock_ms};

const CONTINUE_USAGE: &str =
    "usage: kiln-web-service continue-implementation-campaign --profile PROFILE --campaign-id ID";
const WAKE_USAGE: &str = "usage: kiln-web-service wake-implementation-campaign --database DB --campaign-id ID --kind KIND [--observed-version N]";

// r[impl kiln.implementation.campaign.continuation]
/// Run at most the profile's batch bound through the native drive shell and
/// report the next step: drive again, wait until a time or a hint, or stop.
pub fn continue_cli_args(arguments: &[String]) -> Result<String, String> {
    let [profile_flag, profile, campaign_flag, campaign_id] = arguments else {
        return Err(CONTINUE_USAGE.into());
    };
    if profile_flag != "--profile" || campaign_flag != "--campaign-id" {
        return Err(CONTINUE_USAGE.into());
    }
    let (mut store, mut consumer) = open_native(Path::new(profile), campaign_id, None)?;
    let policy = consumer
        .profile
        .continuation
        .unwrap_or(ContinuationPolicy::DEFAULT);
    let maximum_operations = consumer.profile.maximum_operations;
    let mut clock = wall_clock_ms;
    let run = continue_bounded(
        &mut store,
        campaign_id,
        &policy,
        maximum_operations,
        &mut clock,
        &mut consumer,
    )?;
    let version = store
        .version(campaign_id)?
        .ok_or("campaign disappeared during continuation")?;
    let state = store
        .inspect(campaign_id)?
        .ok_or("campaign disappeared during continuation")?;
    let evaluation = state.evaluate();
    serde_json::to_string(&serde_json::json!({
        "campaign": state,
        "version": version,
        "decision": evaluation.decision,
        "basis": evaluation.basis,
        "holds": evaluation.holds,
        "batches": run.batches,
        "next": run.next,
        "continuation": run.record,
        "continuation_version": run.record_version,
    }))
    .map_err(text)
}

/// Coalesce one wake hint into an initialized campaign's continuation record.
pub fn wake_cli_args(arguments: &[String]) -> Result<String, String> {
    let (required, observed_version) = match arguments {
        [required @ .., flag, version] if arguments.len() == 8 && flag == "--observed-version" => (
            required,
            Some(
                version
                    .parse::<u64>()
                    .map_err(|_| "observed version must be an unsigned integer")?,
            ),
        ),
        required => (required, None),
    };
    let [
        database_flag,
        database,
        campaign_flag,
        campaign_id,
        kind_flag,
        kind,
    ] = required
    else {
        return Err(WAKE_USAGE.into());
    };
    if database_flag != "--database" || campaign_flag != "--campaign-id" || kind_flag != "--kind" {
        return Err(WAKE_USAGE.into());
    }
    let database = PathBuf::from(database);
    if !database.is_absolute() {
        return Err("campaign database path must be absolute".into());
    }
    if campaign_id.trim().is_empty() || campaign_id.len() > MAX_ID_BYTES {
        return Err("campaign identity is malformed".into());
    }
    let kind = WakeKind::parse(kind).map_err(show)?;
    // The read-only handle never creates a database or schema.
    CampaignObservation::open(&database)?
        .inspect(campaign_id)?
        .ok_or("canonical campaign is not initialized")?;
    let mut store = CampaignStore::open(&database)?;
    let (record, version) = store.record_wake(
        campaign_id,
        WakeHint {
            kind,
            observed_version,
        },
    )?;
    serde_json::to_string(&serde_json::json!({
        "continuation": record,
        "continuation_version": version,
    }))
    .map_err(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::campaign::{
        CAMPAIGN_SCHEMA, CampaignContract, CompletionGoal, MemberBinding, MemberSelector,
        WorkflowProfile,
    };
    use kiln_core::types::ContentId;

    const CAMPAIGN: &str = "continuation";

    fn id() -> ContentId {
        ContentId::from_blake3_hex("a".repeat(64)).unwrap_or_else(|error| unreachable!("{error:?}"))
    }

    fn started(directory: &Path) -> PathBuf {
        let database = directory.join("campaign.sqlite");
        let mut store =
            CampaignStore::open(&database).unwrap_or_else(|error| unreachable!("{error}"));
        store
            .start(CampaignContract {
                schema: CAMPAIGN_SCHEMA.into(),
                campaign_id: CAMPAIGN.into(),
                revision: 1,
                repository: "repo".into(),
                members: vec![MemberBinding {
                    selector: MemberSelector::WholeChange {
                        change: "change-a".into(),
                    },
                    profile: WorkflowProfile::SpecDriven,
                    profile_version: id(),
                    repository_instructions: id(),
                    policy: id(),
                    required_checks: Vec::new(),
                    required_reviews: Vec::new(),
                    integration_policy: id(),
                    goal: CompletionGoal::VerifiedCandidate,
                    prerequisite_inclusion: false,
                    context: Vec::new(),
                }],
                max_concurrency: 1,
                max_outstanding_decisions: 1,
                parent_budget_id: id(),
                agent_definition: id(),
                require_one_writer_per_repository: true,
            })
            .unwrap_or_else(|error| unreachable!("{error}"));
        database
    }

    fn wake(
        database: &Path,
        campaign_id: &str,
        kind: &str,
        observed: Option<&str>,
    ) -> Result<String, String> {
        let mut arguments = vec![
            "--database".to_owned(),
            database.display().to_string(),
            "--campaign-id".to_owned(),
            campaign_id.to_owned(),
            "--kind".to_owned(),
            kind.to_owned(),
        ];
        if let Some(version) = observed {
            arguments.extend(["--observed-version".to_owned(), version.to_owned()]);
        }
        wake_cli_args(&arguments)
    }

    #[test]
    // r[verify kiln.implementation.campaign.continuation.scenario.coalesce]
    fn wake_hints_coalesce_into_the_initialized_campaign_record() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let database = started(directory.path());
        for (kind, observed) in [
            ("verification", Some("3")),
            ("verification", Some("2")),
            ("capacity", None),
        ] {
            wake(&database, CAMPAIGN, kind, observed)
                .unwrap_or_else(|error| unreachable!("{error}"));
        }
        let store = CampaignStore::open(&database).unwrap_or_else(|error| unreachable!("{error}"));
        let (record, version) = store
            .continuation(CAMPAIGN)
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(version, 3);
        assert_eq!(record.demand_sequence, 3);
        assert_eq!(
            record.demand.into_iter().collect::<Vec<_>>(),
            [WakeKind::Verification, WakeKind::Capacity]
        );
        assert_eq!(record.highest_hinted_version, Some(3));
        assert_eq!(record.batches, 0, "a hint never runs or records a batch");
    }

    #[test]
    fn wake_refuses_malformed_or_unknown_targets_without_creating_a_database() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let database = started(directory.path());
        assert!(wake(&database, CAMPAIGN, "deploy", None).is_err());
        assert!(wake(&database, CAMPAIGN, "review", Some("-1")).is_err());
        assert!(wake(&database, "other", "review", None).is_err());
        assert!(wake(Path::new("relative.sqlite"), CAMPAIGN, "review", None).is_err());
        let missing = directory.path().join("missing.sqlite");
        assert!(wake(&missing, CAMPAIGN, "review", None).is_err());
        assert!(
            !missing.exists(),
            "a wake never creates a campaign database"
        );
        assert_eq!(
            wake_cli_args(&["--kind".into(), "review".into()]),
            Err(WAKE_USAGE.into())
        );
        let store = CampaignStore::open(&database).unwrap_or_else(|error| unreachable!("{error}"));
        let (record, version) = store
            .continuation(CAMPAIGN)
            .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!((record.demand_sequence, version), (0, 0));
    }

    #[test]
    fn continue_requires_its_exact_arguments_and_a_readable_profile() {
        assert_eq!(continue_cli_args(&[]), Err(CONTINUE_USAGE.into()));
        assert_eq!(
            continue_cli_args(&[
                "--campaign-id".into(),
                CAMPAIGN.into(),
                "--profile".into(),
                "/nonexistent/profile.json".into(),
            ]),
            Err(CONTINUE_USAGE.into())
        );
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let missing = directory.path().join("profile.json");
        assert!(
            continue_cli_args(&[
                "--profile".into(),
                missing.display().to_string(),
                "--campaign-id".into(),
                CAMPAIGN.into(),
            ])
            .is_err()
        );
    }
}
