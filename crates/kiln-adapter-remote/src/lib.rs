use kiln_core::domain::Outcome;
use kiln_core::remote::{
    RemoteAction, RemoteCapability, RemoteObservation, RemoteProfile, admit_action,
    admit_remote_completion,
};
use kiln_core::types::{CoreResult, Diagnostic, ExternalRef};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportResult {
    Observation(Box<RemoteObservation>),
    DisconnectedAfterAcceptance {
        action: ExternalRef,
        operation: Option<ExternalRef>,
    },
}

pub trait RemoteTransport {
    fn upload(&mut self, action: &RemoteAction) -> CoreResult<()>;
    fn execute(&mut self, action: &RemoteAction) -> CoreResult<TransportResult>;
    fn cancel(&mut self, operation: &ExternalRef) -> CoreResult<TransportResult>;
}

pub trait RemoteLifecycleTransport: RemoteTransport {
    fn poll(&mut self, operation: &ExternalRef) -> CoreResult<TransportResult>;
    fn wait(&mut self, operation: &ExternalRef) -> CoreResult<TransportResult>;
    fn download(&mut self, output: &ExternalRef, max_bytes: usize) -> CoreResult<Vec<u8>>;
    fn read_log(&mut self, log: &ExternalRef, max_bytes: usize) -> CoreResult<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationMode {
    Poll,
    Wait,
}

pub fn observe_operation<T: RemoteLifecycleTransport>(
    profile: &RemoteProfile,
    operation: &ExternalRef,
    mode: ObservationMode,
    transport: &mut T,
) -> CoreResult<TransportResult> {
    let capability = match mode {
        ObservationMode::Poll => RemoteCapability::Poll,
        ObservationMode::Wait => RemoteCapability::Wait,
    };
    if !profile.capabilities.contains(&capability) {
        return Err(Diagnostic::new(
            "remote_observation_unsupported",
            "remote.capabilities",
            "remote provider does not declare the selected observation mode",
        ));
    }
    operation.validate()?;
    match mode {
        ObservationMode::Poll => transport.poll(operation),
        ObservationMode::Wait => transport.wait(operation),
    }
}

pub fn download_output<T: RemoteLifecycleTransport>(
    profile: &RemoteProfile,
    output: &ExternalRef,
    max_bytes: usize,
    transport: &mut T,
) -> CoreResult<Vec<u8>> {
    if !profile.capabilities.contains(&RemoteCapability::Download) {
        return Err(Diagnostic::new(
            "remote_download_unsupported",
            "remote.capabilities",
            "remote provider does not declare output download",
        ));
    }
    output.validate()?;
    transport.download(output, max_bytes)
}

pub fn read_bounded_log<T: RemoteLifecycleTransport>(
    profile: &RemoteProfile,
    log: &ExternalRef,
    transport: &mut T,
) -> CoreResult<Vec<u8>> {
    if !profile.capabilities.contains(&RemoteCapability::LogStream) {
        return Err(Diagnostic::new(
            "remote_log_unsupported",
            "remote.capabilities",
            "remote provider does not declare bounded log access",
        ));
    }
    log.validate()?;
    transport.read_log(log, profile.max_log_bytes)
}

pub fn dispatch<T: RemoteTransport>(
    profile: &RemoteProfile,
    action: &RemoteAction,
    required_digest: &ExternalRef,
    encoded_bytes: usize,
    transport: &mut T,
) -> CoreResult<RemoteObservation> {
    admit_action(profile, action, required_digest, encoded_bytes)?;
    if !profile.capabilities.contains(&RemoteCapability::CasUpload) {
        return Err(Diagnostic::new(
            "remote_upload_unsupported",
            "remote.capabilities",
            "remote provider does not support CAS upload",
        ));
    }
    transport.upload(action)?;
    match transport.execute(action)? {
        TransportResult::Observation(observation) => {
            let _outcome = admit_remote_completion(&observation)?;
            Ok(*observation)
        }
        TransportResult::DisconnectedAfterAcceptance { action, operation } => {
            Ok(RemoteObservation {
                action,
                operation,
                cache_result: None,
                execution: None,
                worker: None,
                logs: Vec::new(),
                outcome: Outcome::Unknown,
                required_outputs_present: false,
            })
        }
    }
}

pub fn cancel_operation<T: RemoteTransport>(
    profile: &RemoteProfile,
    operation: &ExternalRef,
    transport: &mut T,
) -> CoreResult<TransportResult> {
    if !profile.capabilities.contains(&RemoteCapability::Cancel) {
        return Err(Diagnostic::new(
            "remote_cancel_unsupported",
            "remote.capabilities",
            "remote provider does not declare cancellation",
        ));
    }
    operation.validate()?;
    transport.cancel(operation)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kiln_core::types::{AttemptId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId};

    use super::*;

    const MAX_ACTION_BYTES: usize = 4096;
    const MAX_LOG_BYTES: usize = 4096;
    const ENCODED_BYTES: usize = 128;

    struct DisconnectingTransport;

    impl RemoteTransport for DisconnectingTransport {
        fn upload(&mut self, _action: &RemoteAction) -> CoreResult<()> {
            Ok(())
        }

        fn execute(&mut self, _action: &RemoteAction) -> CoreResult<TransportResult> {
            Ok(TransportResult::DisconnectedAfterAcceptance {
                action: ExternalRef::new("reapi", "action", "action")?,
                operation: Some(ExternalRef::new("reapi", "operation", "operation")?),
            })
        }

        fn cancel(&mut self, _operation: &ExternalRef) -> CoreResult<TransportResult> {
            Err(Diagnostic::new(
                "unsupported",
                "cancel",
                "fixture does not cancel",
            ))
        }
    }

    fn content(value: char) -> ContentId {
        ContentId::from_blake3_hex(value.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn disconnect_after_acceptance_stays_unknown() {
        let digest =
            ExternalRef::new("reapi", "digest", "sha256").unwrap_or_else(|_| unreachable!());
        let profile = RemoteProfile {
            protocol_version: ExternalRef::new("reapi", "version", "v2")
                .unwrap_or_else(|_| unreachable!()),
            provider: ProviderProfileId::new("remote").unwrap_or_else(|_| unreachable!()),
            digest_functions: BTreeSet::from([digest.clone()]),
            capabilities: BTreeSet::from([RemoteCapability::CasUpload, RemoteCapability::Execute]),
            max_action_bytes: MAX_ACTION_BYTES,
            max_log_bytes: MAX_LOG_BYTES,
        };
        let action = RemoteAction {
            effect_id: EffectId::new("effect").unwrap_or_else(|_| unreachable!()),
            attempt_id: AttemptId::new("attempt").unwrap_or_else(|_| unreachable!()),
            pipeline_revision: PipelineRevisionId::new("pipeline")
                .unwrap_or_else(|_| unreachable!()),
            source_or_candidate: content('a'),
            command: content('b'),
            environment: content('c'),
            platform: content('d'),
            input_root: ExternalRef::new("reapi", "input-root", "root")
                .unwrap_or_else(|_| unreachable!()),
            canonical_input_root: content('e'),
            declared_outputs: Vec::new(),
            timeout_policy: content('f'),
            provider: profile.provider.clone(),
        };
        let observation = dispatch(
            &profile,
            &action,
            &digest,
            ENCODED_BYTES,
            &mut DisconnectingTransport,
        );
        assert_eq!(observation.map(|value| value.outcome), Ok(Outcome::Unknown));
    }

    #[test]
    fn missing_digest_fails_before_upload() {
        let profile = RemoteProfile {
            protocol_version: ExternalRef::new("reapi", "version", "v2")
                .unwrap_or_else(|_| unreachable!()),
            provider: ProviderProfileId::new("remote").unwrap_or_else(|_| unreachable!()),
            digest_functions: BTreeSet::new(),
            capabilities: BTreeSet::from([RemoteCapability::CasUpload, RemoteCapability::Execute]),
            max_action_bytes: MAX_ACTION_BYTES,
            max_log_bytes: MAX_LOG_BYTES,
        };
        assert!(profile.digest_functions.is_empty());
    }
}
