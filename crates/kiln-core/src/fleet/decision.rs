//! Operator decisions for sessions that need a human.
//!
//! A decision records one question for one session and attempt. Only an
//! operator machine may answer, once; the answer is delivered to the session
//! as a prompt. A decision for a session that has ended closes unanswered.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use super::session::SessionLedger;
use super::{bounded_token, content_identity, push_field, refusal};
use crate::types::{ContentId, CoreResult};

pub const FLEET_DECISION_SCHEMA: &str = "kiln.fleet-decision.v1";
pub const MAX_QUESTION_BYTES: usize = 2048;
pub const MAX_ANSWER_BYTES: usize = 4096;
pub const MAX_ANSWERER_BYTES: usize = 128;
/// Phrase identifying a stale-observation decision raised by the tick planner.
pub const STALE_OBSERVATION_QUESTION_MARKER: &str = "has been unobserved for ";

/// Operator action parsed from an answer to a stale-observation decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaleDecisionAction {
    KeepWaiting,
    Cancel,
    LetLeaseLapse,
}

/// Whether `decision` was raised for a stale session observation.
#[must_use]
pub fn is_stale_observation_decision(decision: &Decision) -> bool {
    is_stale_observation_question(&decision.question)
}

/// Whether `question` is a stale-observation decision prompt.
#[must_use]
pub fn is_stale_observation_question(question: &str) -> bool {
    question.contains(STALE_OBSERVATION_QUESTION_MARKER)
}

/// Parse an operator answer to a stale-observation decision.
#[must_use]
pub fn parse_stale_decision_action(text: &str) -> Option<StaleDecisionAction> {
    let normalized = text.trim().to_ascii_lowercase();
    if normalized.starts_with("keep waiting") || normalized == "wait" {
        Some(StaleDecisionAction::KeepWaiting)
    } else if normalized.starts_with("cancel") {
        Some(StaleDecisionAction::Cancel)
    } else if normalized.starts_with("let the lease lapse")
        || normalized.starts_with("let lease lapse")
        || normalized == "lapse"
    {
        Some(StaleDecisionAction::LetLeaseLapse)
    } else {
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    pub text: String,
    /// The authenticated operator machine that answered.
    pub by: String,
    pub at_ms: u64,
}

/// One raised decision.
///
/// `closed_ms` is added beside the designed fields: it records when a
/// decision for an ended session closed without an answer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub identity: ContentId,
    pub session: ContentId,
    pub attempt: ContentId,
    pub question: String,
    pub raised_ms: u64,
    pub answer: Option<Answer>,
    #[serde(default)]
    pub closed_ms: Option<u64>,
}

impl Decision {
    /// Neither answered nor closed.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.answer.is_none() && self.closed_ms.is_none()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionLedger {
    pub decisions: BTreeMap<ContentId, Decision>,
}

impl DecisionLedger {
    /// The open decision for `session`, if any. There is at most one.
    #[must_use]
    pub fn open_for_session(&self, session: &ContentId) -> Option<&Decision> {
        self.decisions
            .values()
            .find(|decision| &decision.session == session && decision.is_open())
    }

    /// The most recently raised answered decision for `session`.
    #[must_use]
    pub fn latest_answered(&self, session: &ContentId) -> Option<&Decision> {
        self.decisions
            .values()
            .filter(|decision| &decision.session == session && decision.answer.is_some())
            .max_by(|left, right| {
                left.raised_ms
                    .cmp(&right.raised_ms)
                    .then_with(|| left.identity.cmp(&right.identity))
            })
    }
}

/// Deterministic decision identity over the session, attempt, question and
/// raise time.
///
/// # Errors
/// Returns a diagnostic only when a field exceeds the encoding bound.
pub fn decision_identity(
    session: &ContentId,
    attempt: &ContentId,
    question: &str,
    raised_ms: u64,
) -> CoreResult<ContentId> {
    let mut bytes = Vec::new();
    push_field(&mut bytes, FLEET_DECISION_SCHEMA, "decision.identity")?;
    push_field(&mut bytes, session.as_str(), "decision.identity")?;
    push_field(&mut bytes, attempt.as_str(), "decision.identity")?;
    push_field(&mut bytes, question, "decision.identity")?;
    bytes.extend_from_slice(&raised_ms.to_be_bytes());
    content_identity(&bytes)
}

// r[impl kiln.fleet.decisions]
/// Raise a decision for a session. One decision per session is open at a
/// time.
///
/// # Errors
/// Refuses malformed identities, an empty, oversized or NUL-bearing question,
/// and a session that already has an open decision.
pub fn raise(
    ledger: &DecisionLedger,
    session: &ContentId,
    attempt: &ContentId,
    question: &str,
    now_ms: u64,
) -> CoreResult<(DecisionLedger, Decision)> {
    session.validate()?;
    attempt.validate()?;
    if !bounded_text(question, MAX_QUESTION_BYTES) {
        return Err(refusal(
            "fleet_decision_question",
            "question",
            "question is empty, oversized or contains NUL",
        ));
    }
    if ledger.open_for_session(session).is_some() {
        return Err(refusal(
            "fleet_decision_open",
            "session",
            "the session already has an open decision",
        ));
    }
    let identity = decision_identity(session, attempt, question, now_ms)?;
    if ledger.decisions.contains_key(&identity) {
        return Err(refusal(
            "fleet_decision_identity",
            "decision.identity",
            "decision identity already exists",
        ));
    }
    let decision = Decision {
        identity: identity.clone(),
        session: session.clone(),
        attempt: attempt.clone(),
        question: question.to_string(),
        raised_ms: now_ms,
        answer: None,
        closed_ms: None,
    };
    let mut next = ledger.clone();
    next.decisions.insert(identity, decision.clone());
    Ok((next, decision))
}

// r[impl kiln.fleet.decisions]
/// Record the one operator answer to an open decision and return the prompt
/// text to deliver to its session.
///
/// # Errors
/// Refuses non-operators, unknown decisions, answered or closed decisions,
/// an empty, oversized or NUL-bearing answer and a malformed answerer.
pub fn answer(
    ledger: &DecisionLedger,
    identity: &ContentId,
    text: &str,
    by: &str,
    operator: bool,
    now_ms: u64,
) -> CoreResult<(DecisionLedger, String)> {
    if !operator {
        return Err(refusal(
            "fleet_decision_operator",
            "by",
            "only an operator machine may answer a decision",
        ));
    }
    if !bounded_token(by, MAX_ANSWERER_BYTES) {
        return Err(refusal(
            "fleet_decision_answer",
            "by",
            "answerer is empty, oversized or malformed",
        ));
    }
    if !bounded_text(text, MAX_ANSWER_BYTES) {
        return Err(refusal(
            "fleet_decision_answer",
            "text",
            "answer is empty, oversized or contains NUL",
        ));
    }
    let decision = ledger.decisions.get(identity).ok_or_else(|| {
        refusal(
            "fleet_decision_unknown",
            "identity",
            "no such fleet decision",
        )
    })?;
    if decision.answer.is_some() {
        return Err(refusal(
            "fleet_decision_answered",
            "identity",
            "the decision already has an answer",
        ));
    }
    if decision.closed_ms.is_some() {
        return Err(refusal(
            "fleet_decision_closed",
            "identity",
            "the decision closed because its session ended",
        ));
    }
    let mut answered = decision.clone();
    answered.answer = Some(Answer {
        text: text.to_string(),
        by: by.to_string(),
        at_ms: now_ms,
    });
    let prompt = answer_prompt(&answered).ok_or_else(|| {
        refusal(
            "fleet_decision_answer",
            "text",
            "answer could not be rendered",
        )
    })?;
    let mut next = ledger.clone();
    next.decisions.insert(identity.clone(), answered);
    Ok((next, prompt))
}

/// Close every open decision whose session has ended or is no longer
/// recorded. Returns the next ledger and the closed identities.
#[must_use]
pub fn close_for_ended_sessions(
    ledger: &DecisionLedger,
    sessions: &SessionLedger,
    now_ms: u64,
) -> (DecisionLedger, Vec<ContentId>) {
    let mut next = ledger.clone();
    let mut closed = Vec::new();
    for decision in next.decisions.values_mut() {
        let ended = sessions
            .sessions
            .get(&decision.session)
            .is_none_or(|session| session.state.is_ended());
        if decision.is_open() && ended {
            decision.closed_ms = Some(now_ms);
            closed.push(decision.identity.clone());
        }
    }
    (next, closed)
}

/// Close any open stale-observation decision for `session` once a fresh
/// observation arrives. Non-stale decisions remain open.
#[must_use]
pub fn close_stale_for_session(
    ledger: &DecisionLedger,
    session: &ContentId,
    now_ms: u64,
) -> (DecisionLedger, Vec<ContentId>) {
    let mut next = ledger.clone();
    let mut closed = Vec::new();
    for decision in next.decisions.values_mut() {
        if &decision.session == session
            && decision.is_open()
            && is_stale_observation_decision(decision)
        {
            decision.closed_ms = Some(now_ms);
            closed.push(decision.identity.clone());
        }
    }
    (next, closed)
}

/// The prompt that delivers an answered decision to its session, or `None`
/// while it has no answer. The question and answer are framed as operator
/// text, distinct from tool output.
#[must_use]
pub fn answer_prompt(decision: &Decision) -> Option<String> {
    let answer = decision.answer.as_ref()?;
    Some(format!(
        "Kiln relays an operator answer to the question raised for this attempt.\n\nQuestion:\n{}\n\nOperator answer (from {}):\n{}\n\nContinue the attempt under the same rules. When the work is done, commit on your branch and hand off as instructed.\n",
        decision.question, answer.by, answer.text,
    ))
}

fn bounded_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::session::{FleetSession, Observation, Placement, bind, observe};

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }

    fn sessions() -> (SessionLedger, ContentId) {
        let session = FleetSession::placing(
            Placement {
                attempt: content('a'),
                fence: 3,
                work: content('f'),
                work_revision: 1,
                machine: "aspen3".to_string(),
                kind: "omp".to_string(),
            },
            10,
        )
        .unwrap_or_else(|_| unreachable!());
        let identity = session.identity.clone();
        let ledger = bind(&SessionLedger::default(), session).unwrap_or_else(|_| unreachable!());
        (ledger, identity)
    }

    #[test]
    // r[verify kiln.fleet.decisions] r[verify kiln.fleet.decisions.scenario.answer]
    fn only_the_first_operator_answer_is_recorded_and_prompted() {
        let (_, session) = sessions();
        let (ledger, decision) = raise(
            &DecisionLedger::default(),
            &session,
            &content('a'),
            "Which schema version should the store use?",
            100,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(decision.is_open());
        assert_eq!(decision.attempt, content('a'));
        assert_eq!(ledger.open_for_session(&session), Some(&decision));

        let refused = answer(&ledger, &decision.identity, "v2", "aspen1", false, 110);
        assert_eq!(
            refused.err().map(|diagnostic| diagnostic.code),
            Some("fleet_decision_operator".to_string())
        );

        let (answered, prompt) = answer(
            &ledger,
            &decision.identity,
            "Use v2.",
            "britton-desktop",
            true,
            120,
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(prompt.contains("Which schema version should the store use?"));
        assert!(prompt.contains("Use v2."));
        assert!(prompt.contains("britton-desktop"));
        let recorded = answered
            .decisions
            .get(&decision.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            recorded.answer,
            Some(Answer {
                text: "Use v2.".to_string(),
                by: "britton-desktop".to_string(),
                at_ms: 120,
            })
        );
        assert_eq!(answer_prompt(recorded), Some(prompt));
        assert_eq!(answered.latest_answered(&session), Some(recorded));
        assert!(answered.open_for_session(&session).is_none());

        let second = answer(
            &answered,
            &decision.identity,
            "Use v3.",
            "britton-desktop",
            true,
            130,
        );
        assert_eq!(
            second.err().map(|diagnostic| diagnostic.code),
            Some("fleet_decision_answered".to_string())
        );
        assert_eq!(
            answer(&answered, &content('e'), "x", "britton-desktop", true, 130)
                .err()
                .map(|diagnostic| diagnostic.code),
            Some("fleet_decision_unknown".to_string())
        );
        for (text, by) in [
            ("", "britton-desktop"),
            ("x", "britton desktop"),
            ("x\0", "op"),
        ] {
            assert_eq!(
                answer(&ledger, &decision.identity, text, by, true, 130)
                    .err()
                    .map(|diagnostic| diagnostic.code),
                Some("fleet_decision_answer".to_string())
            );
        }
    }

    #[test]
    // r[verify kiln.fleet.decisions]
    fn one_open_decision_per_session() {
        let (_, session) = sessions();
        let (ledger, first) = raise(&DecisionLedger::default(), &session, &content('a'), "q1", 1)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(
            raise(&ledger, &session, &content('a'), "q2", 2)
                .err()
                .map(|diagnostic| diagnostic.code),
            Some("fleet_decision_open".to_string())
        );
        assert_eq!(
            raise(&ledger, &content('b'), &content('a'), "", 2)
                .err()
                .map(|diagnostic| diagnostic.code),
            Some("fleet_decision_question".to_string())
        );
        assert!(
            raise(
                &ledger,
                &content('b'),
                &content('a'),
                &"q".repeat(MAX_QUESTION_BYTES + 1),
                2
            )
            .is_err()
        );
        // After an answer, a new decision may be raised.
        let (ledger, _) = answer(&ledger, &first.identity, "ok", "op", true, 3)
            .unwrap_or_else(|_| unreachable!());
        let (ledger, second) =
            raise(&ledger, &session, &content('a'), "q2", 4).unwrap_or_else(|_| unreachable!());
        assert_eq!(ledger.open_for_session(&session), Some(&second));
        assert_eq!(
            ledger
                .latest_answered(&session)
                .map(|decision| &decision.identity),
            Some(&first.identity)
        );
    }

    #[test]
    // r[verify kiln.fleet.decisions]
    fn decisions_for_ended_sessions_close_without_delivery() {
        let (sessions, session) = sessions();
        let (ledger, open) = raise(&DecisionLedger::default(), &session, &content('a'), "q", 1)
            .unwrap_or_else(|_| unreachable!());
        let (orphan_ledger, orphan) =
            raise(&ledger, &content('c'), &content('d'), "q", 1).unwrap_or_else(|_| unreachable!());
        // The live session keeps its decision; the unknown session's closes.
        let (kept, closed) = close_for_ended_sessions(&orphan_ledger, &sessions, 5);
        assert_eq!(closed, alloc::vec![orphan.identity.clone()]);
        assert!(kept.open_for_session(&session).is_some());

        let ended = observe(
            &sessions,
            &session,
            Observation::StartFailed {
                reason: "herdr unavailable".to_string(),
            },
            6,
        )
        .unwrap_or_else(|_| unreachable!());
        let (closed_ledger, closed) = close_for_ended_sessions(&kept, &ended, 7);
        assert_eq!(closed, alloc::vec![open.identity.clone()]);
        let record = closed_ledger
            .decisions
            .get(&open.identity)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(record.closed_ms, Some(7));
        assert!(record.answer.is_none());
        assert_eq!(answer_prompt(record), None);
        assert_eq!(
            answer(&closed_ledger, &open.identity, "late", "op", true, 8)
                .err()
                .map(|diagnostic| diagnostic.code),
            Some("fleet_decision_closed".to_string())
        );
        // Closing again changes nothing.
        let (again, none) = close_for_ended_sessions(&closed_ledger, &ended, 9);
        assert!(none.is_empty());
        assert_eq!(again, closed_ledger);
    }
}
