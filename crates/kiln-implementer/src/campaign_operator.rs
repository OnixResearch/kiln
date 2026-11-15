//! Bounded command-file operator protocol for campaign control.
use kiln_core::implementation::campaign::{CampaignContract, CampaignState};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

use crate::campaign::CampaignStore;

pub const MAX_CAMPAIGN_COMMAND_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum CampaignCommand {
    Start {
        contract: CampaignContract,
    },
    Inspect {
        campaign_id: String,
    },
    Cancel {
        campaign_id: String,
        expected_version: u64,
    },
    Continue {
        campaign_id: String,
        expected_version: u64,
    },
    Reconcile {
        campaign_id: String,
    },
    IssueDecision {
        campaign_id: String,
        expected_version: u64,
        identity: kiln_core::types::ContentId,
    },
    ResolveDecision {
        campaign_id: String,
        expected_version: u64,
        identity: kiln_core::types::ContentId,
        work_identity: Option<kiln_core::types::ContentId>,
        policy_identity: kiln_core::types::ContentId,
        candidate_identity: Option<kiln_core::types::ContentId>,
        answer: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignCommandResult {
    pub store_version: Option<u64>,
    pub campaign: Option<CampaignState>,
    pub pending_effects: Vec<kiln_core::implementation::campaign::PendingEffect>,
    pub decision: Option<kiln_core::implementation::campaign::CampaignDecision>,
}

/// Apply one bounded operator command. Reconcile is inspection/action planning;
/// actual owner observations must be submitted through their canonical API.
pub fn apply_command_file(path: &Path, database: &Path) -> Result<CampaignCommandResult, String> {
    let metadata = std::fs::metadata(path).map_err(text)?;
    if metadata.len() == 0 || metadata.len() > MAX_CAMPAIGN_COMMAND_BYTES {
        return Err("campaign command file is empty or over its bound".into());
    }
    let bytes = std::fs::read(path).map_err(text)?;
    let command: CampaignCommand = serde_json::from_slice(&bytes).map_err(text)?;
    apply(command, database)
}

/// Command implementation is separated from filesystem acquisition for typed tests.
pub fn apply(command: CampaignCommand, database: &Path) -> Result<CampaignCommandResult, String> {
    let mut store = CampaignStore::open(database)?;
    match command {
        CampaignCommand::Start { contract } => {
            let campaign = store.start(contract)?;
            Ok(CampaignCommandResult {
                store_version: store.version(&campaign.contract.campaign_id)?,
                decision: Some(
                    campaign
                        .next_action()
                        .map_err(|e| format!("{}: {}", e.code, e.message))?,
                ),
                campaign: Some(campaign),
                pending_effects: Vec::new(),
            })
        }
        CampaignCommand::Inspect { campaign_id } => {
            let campaign = store.inspect(&campaign_id)?.ok_or("campaign not found")?;
            let decision = store.decide(&campaign_id).ok();
            Ok(CampaignCommandResult {
                store_version: store.version(&campaign_id)?,
                campaign: Some(campaign),
                pending_effects: Vec::new(),
                decision,
            })
        }
        CampaignCommand::Cancel {
            campaign_id,
            expected_version,
        } => {
            let campaign = store.cancel(&campaign_id, expected_version)?;
            Ok(CampaignCommandResult {
                store_version: store.version(&campaign_id)?,
                campaign: Some(campaign),
                pending_effects: Vec::new(),
                decision: Some(store.decide(&campaign_id)?),
            })
        }
        CampaignCommand::Continue {
            campaign_id,
            expected_version,
        } => {
            let campaign = store.continue_admission(&campaign_id, expected_version)?;
            Ok(CampaignCommandResult {
                store_version: store.version(&campaign_id)?,
                campaign: Some(campaign),
                pending_effects: Vec::new(),
                decision: Some(store.decide(&campaign_id)?),
            })
        }
        CampaignCommand::IssueDecision {
            campaign_id,
            expected_version,
            identity,
        } => {
            let campaign = store.issue_decision(&campaign_id, expected_version, &identity)?;
            Ok(CampaignCommandResult {
                store_version: store.version(&campaign_id)?,
                decision: Some(store.decide(&campaign_id)?),
                campaign: Some(campaign),
                pending_effects: Vec::new(),
            })
        }
        CampaignCommand::ResolveDecision {
            campaign_id,
            expected_version,
            identity,
            work_identity,
            policy_identity,
            candidate_identity,
            answer,
        } => {
            let campaign = store.resolve_decision(
                &campaign_id,
                expected_version,
                crate::campaign::CampaignDecisionAnswer {
                    identity: &identity,
                    work: work_identity.as_ref(),
                    policy: &policy_identity,
                    candidate: candidate_identity.as_ref(),
                    answer: &answer,
                },
            )?;
            Ok(CampaignCommandResult {
                store_version: store.version(&campaign_id)?,
                decision: Some(store.decide(&campaign_id)?),
                campaign: Some(campaign),
                pending_effects: Vec::new(),
            })
        }
        CampaignCommand::Reconcile { campaign_id } => {
            let pending_effects = store.recover_pending(&campaign_id)?;
            let decision = Some(store.decide(&campaign_id)?);
            let campaign = store.inspect(&campaign_id)?;
            Ok(CampaignCommandResult {
                store_version: store.version(&campaign_id)?,
                campaign,
                pending_effects,
                decision,
            })
        }
    }
}

/// Adapter for the existing CLI's parsed option map and `--database` flag.
pub fn cli_dispatch(flags: &BTreeMap<String, String>) -> Result<CampaignCommandResult, String> {
    let command_file = flags
        .get("--command-file")
        .ok_or("missing --command-file")?;
    let database = flags.get("--database").ok_or("missing --database")?;
    apply_command_file(Path::new(command_file), Path::new(database))
}
fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_json_rejects_unknown_fields() {
        let parsed = serde_json::from_str::<CampaignCommand>(
            r#"{"command":"cancel","campaign_id":"c","expected_version":1,"force":true}"#,
        );
        assert!(parsed.is_err());
    }
}
