use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::domain::Outcome;
use crate::types::{
    AttemptId, ContentId, CoreResult, Diagnostic, EffectId, ExternalRef, PipelineRevisionId,
    ProviderProfileId,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum RemoteCapability {
    CasUpload,
    Execute,
    Poll,
    Wait,
    Cancel,
    CacheLookup,
    Download,
    LogStream,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemoteProfile {
    pub protocol_version: ExternalRef,
    pub provider: ProviderProfileId,
    pub digest_functions: BTreeSet<ExternalRef>,
    pub capabilities: BTreeSet<RemoteCapability>,
    pub max_action_bytes: usize,
    pub max_log_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemoteAction {
    pub effect_id: EffectId,
    pub attempt_id: AttemptId,
    pub pipeline_revision: PipelineRevisionId,
    pub source_or_candidate: ContentId,
    pub command: ContentId,
    pub environment: ContentId,
    pub platform: ContentId,
    pub input_root: ExternalRef,
    pub canonical_input_root: ContentId,
    pub declared_outputs: Vec<ExternalRef>,
    pub timeout_policy: ContentId,
    pub provider: ProviderProfileId,
}

pub fn admit_action(
    profile: &RemoteProfile,
    action: &RemoteAction,
    required_digest: &ExternalRef,
    encoded_bytes: usize,
) -> CoreResult<()> {
    profile.protocol_version.validate()?;
    if profile.provider != action.provider {
        return Err(Diagnostic::new(
            "remote_provider_mismatch",
            "remote_action.provider",
            "remote action provider does not match the selected profile",
        ));
    }
    if !profile.capabilities.contains(&RemoteCapability::Execute) {
        return Err(Diagnostic::new(
            "remote_execute_unsupported",
            "remote.capabilities",
            "remote provider does not support execution",
        ));
    }
    if !profile.digest_functions.contains(required_digest) {
        return Err(Diagnostic::new(
            "remote_digest_unsupported",
            "remote.digest_functions",
            "remote provider lacks the required digest function",
        ));
    }
    if encoded_bytes > profile.max_action_bytes {
        return Err(Diagnostic::new(
            "remote_action_bound",
            "remote_action",
            "remote action exceeds its named byte bound",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemoteObservation {
    pub action: ExternalRef,
    pub operation: Option<ExternalRef>,
    pub cache_result: Option<ExternalRef>,
    pub execution: Option<ExternalRef>,
    pub worker: Option<ExternalRef>,
    pub logs: Vec<ExternalRef>,
    pub outcome: Outcome,
    pub required_outputs_present: bool,
}

pub fn admit_remote_completion(observation: &RemoteObservation) -> CoreResult<Outcome> {
    observation.action.validate()?;
    if observation.outcome == Outcome::Success && !observation.required_outputs_present {
        return Err(Diagnostic::new(
            "remote_output_missing",
            "remote.outputs",
            "successful remote observation omits a required output",
        ));
    }
    Ok(observation.outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_without_output_is_not_success() {
        let observation = RemoteObservation {
            action: ExternalRef::new("reapi", "action", "action")
                .unwrap_or_else(|_| unreachable!()),
            operation: None,
            cache_result: Some(
                ExternalRef::new("reapi", "cache-result", "same")
                    .unwrap_or_else(|_| unreachable!()),
            ),
            execution: None,
            worker: None,
            logs: Vec::new(),
            outcome: Outcome::Success,
            required_outputs_present: false,
        };
        assert!(admit_remote_completion(&observation).is_err());
    }

    #[test]
    fn equal_native_text_keeps_distinct_kinds() {
        let operation =
            ExternalRef::new("reapi", "operation", "same").unwrap_or_else(|_| unreachable!());
        let cache =
            ExternalRef::new("reapi", "cache-result", "same").unwrap_or_else(|_| unreachable!());
        assert_ne!(operation, cache);
    }
}
