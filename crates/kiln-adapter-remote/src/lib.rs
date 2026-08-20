use kiln_core::remote::{
    RemoteAction, RemoteCapability, RemoteObservation, RemoteProfile, admit_action,
    admit_remote_capability, admit_remote_completion, disconnected_remote_observation,
};
use kiln_core::types::ExternalRef;
use kiln_provider_api::{
    PortResult, RemoteExecutionPort, RemoteLifecyclePort, RemoteTransportObservation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationMode {
    Poll,
    Wait,
}

pub fn observe_operation<T: RemoteLifecyclePort>(
    profile: &RemoteProfile,
    operation: &ExternalRef,
    mode: ObservationMode,
    transport: &mut T,
) -> PortResult<RemoteTransportObservation> {
    let capability = match mode {
        ObservationMode::Poll => RemoteCapability::Poll,
        ObservationMode::Wait => RemoteCapability::Wait,
    };
    admit_remote_capability(profile, capability)?;
    operation.validate()?;
    match mode {
        ObservationMode::Poll => transport.poll(operation),
        ObservationMode::Wait => transport.wait(operation),
    }
}

pub fn download_output<T: RemoteLifecyclePort>(
    profile: &RemoteProfile,
    output: &ExternalRef,
    max_bytes: usize,
    transport: &mut T,
) -> PortResult<Vec<u8>> {
    admit_remote_capability(profile, RemoteCapability::Download)?;
    output.validate()?;
    transport.download(output, max_bytes)
}

pub fn read_bounded_log<T: RemoteLifecyclePort>(
    profile: &RemoteProfile,
    log: &ExternalRef,
    transport: &mut T,
) -> PortResult<Vec<u8>> {
    admit_remote_capability(profile, RemoteCapability::LogStream)?;
    log.validate()?;
    transport.read_log(log, profile.max_log_bytes)
}

pub fn dispatch<T: RemoteExecutionPort>(
    profile: &RemoteProfile,
    action: &RemoteAction,
    required_digest: &ExternalRef,
    encoded_bytes: usize,
    transport: &mut T,
) -> PortResult<RemoteObservation> {
    admit_action(profile, action, required_digest, encoded_bytes)?;
    admit_remote_capability(profile, RemoteCapability::CasUpload)?;
    transport.upload(action)?;
    match transport.execute(action)? {
        RemoteTransportObservation::Observation(observation) => {
            let _outcome = admit_remote_completion(&observation)?;
            Ok(*observation)
        }
        RemoteTransportObservation::DisconnectedAfterAcceptance { action, operation } => {
            Ok(disconnected_remote_observation(action, operation)?)
        }
    }
}

pub fn cancel_operation<T: RemoteExecutionPort>(
    profile: &RemoteProfile,
    operation: &ExternalRef,
    transport: &mut T,
) -> PortResult<RemoteTransportObservation> {
    admit_remote_capability(profile, RemoteCapability::Cancel)?;
    operation.validate()?;
    transport.cancel(operation)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kiln_core::domain::Outcome;
    use kiln_core::types::{
        AttemptId, ContentId, Diagnostic, EffectId, PipelineRevisionId, ProviderProfileId,
    };

    use super::*;

    const CONTENT_HEX_LENGTH: usize = 64;
    const MAX_ACTION_BYTES: usize = 4096;
    const MAX_LOG_BYTES: usize = 4096;
    const ENCODED_BYTES: usize = 128;

    #[derive(Default)]
    struct FixtureTransport {
        uploads: usize,
        executions: usize,
        polls: usize,
        waits: usize,
    }

    impl RemoteExecutionPort for FixtureTransport {
        fn upload(&mut self, _action: &RemoteAction) -> PortResult<()> {
            self.uploads += 1;
            Ok(())
        }

        fn execute(&mut self, _action: &RemoteAction) -> PortResult<RemoteTransportObservation> {
            self.executions += 1;
            Ok(RemoteTransportObservation::DisconnectedAfterAcceptance {
                action: ExternalRef::new("reapi", "action", "action")?,
                operation: Some(ExternalRef::new("reapi", "operation", "operation")?),
            })
        }

        fn cancel(&mut self, _operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
            Err(Diagnostic::new("unsupported", "cancel", "fixture does not cancel").into())
        }
    }

    impl RemoteLifecyclePort for FixtureTransport {
        fn poll(&mut self, _operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
            self.polls += 1;
            Ok(RemoteTransportObservation::DisconnectedAfterAcceptance {
                action: ExternalRef::new("reapi", "action", "action")?,
                operation: Some(ExternalRef::new("reapi", "operation", "operation")?),
            })
        }

        fn wait(&mut self, _operation: &ExternalRef) -> PortResult<RemoteTransportObservation> {
            self.waits += 1;
            Ok(RemoteTransportObservation::DisconnectedAfterAcceptance {
                action: ExternalRef::new("reapi", "action", "action")?,
                operation: Some(ExternalRef::new("reapi", "operation", "operation")?),
            })
        }

        fn download(&mut self, _output: &ExternalRef, _max_bytes: usize) -> PortResult<Vec<u8>> {
            Ok(Vec::new())
        }

        fn read_log(&mut self, _log: &ExternalRef, _max_bytes: usize) -> PortResult<Vec<u8>> {
            Ok(Vec::new())
        }
    }

    fn content(value: char) -> ContentId {
        ContentId::from_blake3_hex(value.to_string().repeat(CONTENT_HEX_LENGTH))
            .unwrap_or_else(|_| unreachable!())
    }

    fn digest() -> ExternalRef {
        ExternalRef::new("reapi", "digest", "sha256").unwrap_or_else(|_| unreachable!())
    }

    fn profile(
        digests: BTreeSet<ExternalRef>,
        capabilities: BTreeSet<RemoteCapability>,
    ) -> RemoteProfile {
        RemoteProfile {
            protocol_version: ExternalRef::new("reapi", "version", "v2")
                .unwrap_or_else(|_| unreachable!()),
            provider: ProviderProfileId::new("remote").unwrap_or_else(|_| unreachable!()),
            digest_functions: digests,
            capabilities,
            max_action_bytes: MAX_ACTION_BYTES,
            max_log_bytes: MAX_LOG_BYTES,
        }
    }

    fn action(profile: &RemoteProfile) -> RemoteAction {
        RemoteAction {
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
        }
    }

    // r[verify kiln.providers.error_ownership.scenario.disconnect]
    #[test]
    fn disconnect_after_acceptance_stays_unknown() {
        let required_digest = digest();
        let profile = profile(
            BTreeSet::from([required_digest.clone()]),
            BTreeSet::from([RemoteCapability::CasUpload, RemoteCapability::Execute]),
        );
        let mut transport = FixtureTransport::default();
        let observation = dispatch(
            &profile,
            &action(&profile),
            &required_digest,
            ENCODED_BYTES,
            &mut transport,
        );
        assert_eq!(observation.map(|value| value.outcome), Ok(Outcome::Unknown));
        assert_eq!(transport.uploads, 1);
        assert_eq!(transport.executions, 1);
    }

    // r[verify kiln.providers.boundary_conformance]
    // r[verify kiln.providers.policy_effect_split.scenario.observation]
    #[test]
    fn missing_digest_fails_before_upload() {
        let required_digest = digest();
        let profile = profile(
            BTreeSet::new(),
            BTreeSet::from([RemoteCapability::CasUpload, RemoteCapability::Execute]),
        );
        let mut transport = FixtureTransport::default();
        let result = dispatch(
            &profile,
            &action(&profile),
            &required_digest,
            ENCODED_BYTES,
            &mut transport,
        );
        assert!(result.is_err());
        assert_eq!(transport.uploads, 0);
        assert_eq!(transport.executions, 0);
    }

    #[test]
    fn observation_mode_admission_blocks_unsupported_wait() {
        let profile = profile(BTreeSet::new(), BTreeSet::from([RemoteCapability::Poll]));
        let operation =
            ExternalRef::new("reapi", "operation", "operation").unwrap_or_else(|_| unreachable!());
        let mut transport = FixtureTransport::default();
        assert!(
            observe_operation(&profile, &operation, ObservationMode::Poll, &mut transport).is_ok()
        );
        assert!(
            observe_operation(&profile, &operation, ObservationMode::Wait, &mut transport).is_err()
        );
        assert_eq!(transport.polls, 1);
        assert_eq!(transport.waits, 0);
    }
}
