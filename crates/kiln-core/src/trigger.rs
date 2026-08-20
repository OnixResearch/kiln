use crate::types::{CoreResult, Diagnostic, RunId};

// r[impl kiln.providers.policy_effect_split]
pub fn admit_trigger_poll_bound(max_polls: usize) -> CoreResult<()> {
    if max_polls == 0 {
        return Err(Diagnostic::new(
            "trigger_lifecycle_policy",
            "trigger.policy.max_polls",
            "trigger lifecycle policy requires a positive poll bound",
        ));
    }
    Ok(())
}

pub fn admit_trigger_run_observation(
    accepted_run: &RunId,
    observed_run: &RunId,
    log_url_bytes: Option<usize>,
    max_log_url_bytes: usize,
) -> CoreResult<()> {
    if observed_run != accepted_run {
        return Err(Diagnostic::new(
            "trigger_run_substitution",
            "trigger.run_id",
            "trigger observation changed the accepted Kiln run identity",
        ));
    }
    if log_url_bytes.is_some_and(|bytes| bytes > max_log_url_bytes) {
        return Err(Diagnostic::new(
            "trigger_log_url_bound",
            "trigger.log_url",
            "trigger log URL exceeds its named bound",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_LOG_URL_BYTES: usize = 128;
    const OVERSIZED_LOG_URL_BYTES: usize = MAX_LOG_URL_BYTES + 1;

    fn run(value: &str) -> RunId {
        RunId::new(value).unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn admits_matching_bounded_observation() {
        let accepted = run("accepted");
        assert!(
            admit_trigger_run_observation(
                &accepted,
                &accepted,
                Some(MAX_LOG_URL_BYTES),
                MAX_LOG_URL_BYTES,
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_substitution_and_oversized_log_url() {
        let accepted = run("accepted");
        let substituted = run("substituted");
        assert!(
            admit_trigger_run_observation(&accepted, &substituted, None, MAX_LOG_URL_BYTES,)
                .is_err()
        );
        assert!(
            admit_trigger_run_observation(
                &accepted,
                &accepted,
                Some(OVERSIZED_LOG_URL_BYTES),
                MAX_LOG_URL_BYTES,
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_zero_poll_bound() {
        assert!(admit_trigger_poll_bound(0).is_err());
    }
}
