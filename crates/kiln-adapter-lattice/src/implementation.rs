//! Admission and dispatch for one operator-supplied Lattice implementation request.
//!
//! Kiln binds the request to its own dispatch reservation, attempt/fence, base,
//! repository, output contract and cancellation-only controls. The caller and
//! Lattice remain responsible for the implementation workflow's actual graph and
//! server profile; this module does not create a second workflow engine.

use kiln_codec::identity_from_bytes;
use kiln_core::implementation::{
    AttemptPhase, DispatchReservation, ImplementationAttempt, ImplementationDispatchPlan,
};
use kiln_core::types::Diagnostic;
use kiln_provider_api::{
    InfrastructureFailure, PortFailure, PortResult, WorkflowExchangeOperation,
    WorkflowExchangePort, WorkflowExchangeRequest,
};
use lattice_workflow_contract::{
    EffectClass, GRAPH_SCHEMA, Identity, REQUEST_SCHEMA, WorkflowAccepted, WorkflowRequest,
    request_identity, validate_accepted, validate_request,
};

/// Admit the exact Lattice request for one reserved implementation attempt.
///
/// This is a linkage check only. The caller must provide a workflow request
/// already admitted by the selected Lattice profile and route.
// r[impl kiln.implementation.lattice_dispatch]
pub fn admit_implementation_request(
    dispatch: &ImplementationDispatchPlan,
    reservation: &DispatchReservation,
    attempt: &ImplementationAttempt,
    request: &WorkflowRequest,
) -> PortResult<()> {
    let plan_identity = identity(reservation.plan.as_str())?;
    let attempt_identity = identity(attempt.identity.as_str())?;
    let output_identity = identity(dispatch.expected_output.as_str())?;
    let Some(base) = &attempt.base else {
        return Err(domain(
            "implementation_lattice_base",
            "implementation.request.base",
            "implementation dispatch requires an exact base",
        ));
    };
    if attempt.phase != AttemptPhase::Claimed
        || attempt.dispatch.as_ref() != Some(reservation)
        || request.schema != REQUEST_SCHEMA
        || request.graph.schema != GRAPH_SCHEMA
        || request.identity != request_identity(request)
        || request.idempotency_key != plan_identity
        || request.profile_identity.as_str().is_empty()
        || request.kiln.effect_identity != plan_identity
        || request.kiln.attempt_identity != attempt_identity
        || request.kiln.candidate_identity != output_identity
        || !request.controls.cancel
        || request.controls.pause
        || request.controls.resume
        || request.graph.nodes.is_empty()
        || !request.graph.nodes.iter().all(|node| {
            node.kind.as_str() == "agent"
                && node.adapter_kind.as_str() == "local"
                && node.effect_class == EffectClass::Custom
        })
    {
        return Err(domain(
            "implementation_lattice_request",
            "implementation.request",
            "Lattice request does not bind the exact implementation attempt and route",
        ));
    }
    let Some(source) = &request.source else {
        return Err(domain(
            "implementation_lattice_source",
            "implementation.request.source",
            "implementation dispatch requires a source context",
        ));
    };
    if source.repository.as_str() != dispatch.repository
        || source.revision.as_str() != base.native_id.as_str()
    {
        return Err(domain(
            "implementation_lattice_source",
            "implementation.request.source",
            "Lattice source does not match the repository and frozen base",
        ));
    }
    Ok(())
}

/// Validate and send one implementation request through a Lattice transport.
///
/// A disconnected acceptance remains `Unknown`; callers must reconcile the
/// execution identity before retrying.
// r[impl kiln.implementation.lattice_dispatch]
pub fn dispatch_implementation_request<T: WorkflowExchangePort>(
    transport: &mut T,
    profile: &lattice_workflow_contract::CapabilityProfile,
    request: &WorkflowRequest,
) -> PortResult<WorkflowAccepted> {
    exchange_implementation_request(
        transport,
        profile,
        request,
        WorkflowExchangeOperation::Dispatch,
        "dispatch",
    )
}

/// Reconcile an ambiguous implementation dispatch through Lattice recovery.
///
/// This MUST NOT create a new execution; it accepts only the linked accepted
/// response for the supplied request.
// r[impl kiln.implementation.lattice_dispatch]
pub fn recover_implementation_request<T: WorkflowExchangePort>(
    transport: &mut T,
    profile: &lattice_workflow_contract::CapabilityProfile,
    request: &WorkflowRequest,
) -> PortResult<WorkflowAccepted> {
    exchange_implementation_request(
        transport,
        profile,
        request,
        WorkflowExchangeOperation::RecoverDispatch,
        "recover",
    )
}

fn exchange_implementation_request<T: WorkflowExchangePort>(
    transport: &mut T,
    profile: &lattice_workflow_contract::CapabilityProfile,
    request: &WorkflowRequest,
    operation: WorkflowExchangeOperation,
    label: &str,
) -> PortResult<WorkflowAccepted> {
    validate_request(profile, request).map_err(|_| {
        domain(
            "implementation_lattice_profile",
            "implementation.request.profile",
            "Lattice request is not admitted by the selected profile",
        )
    })?;
    let body = serde_json::to_vec(request).map_err(|_| {
        infrastructure(
            "implementation_lattice_encoding",
            "Lattice request could not be encoded",
        )
    })?;
    let body_identity = identity_from_bytes(&body).map_err(|_| {
        infrastructure(
            "implementation_lattice_encoding",
            "Lattice request body identity could not be computed",
        )
    })?;
    let response = transport.exchange(WorkflowExchangeRequest {
        operation,
        body_identity,
        body,
        max_response_bytes: 4 * 1024 * 1024,
    })?;
    if response.disconnected_after_acceptance {
        return Err(PortFailure::Unknown(InfrastructureFailure {
            code: format!("implementation_lattice_{label}_disconnect"),
            message: format!("Lattice disconnected after {label} of the implementation request"),
        }));
    }
    let accepted: WorkflowAccepted = serde_json::from_slice(&response.body).map_err(|_| {
        infrastructure(
            "implementation_lattice_response",
            "Lattice acceptance response is malformed",
        )
    })?;
    validate_accepted(request, &accepted).map_err(|_| {
        domain(
            "implementation_lattice_acceptance",
            "implementation.acceptance",
            "Lattice acceptance does not match the request",
        )
    })?;
    Ok(accepted)
}

fn identity(value: &str) -> PortResult<Identity> {
    Identity::new(format!("b3:{value}")).map_err(|_| {
        domain(
            "implementation_lattice_identity",
            "implementation.request.identity",
            "implementation identity is not a canonical BLAKE3 identity",
        )
    })
}

fn domain(code: &str, field: &str, message: &str) -> PortFailure {
    PortFailure::Domain(Diagnostic::new(code, field, message))
}

fn infrastructure(code: &str, message: &str) -> PortFailure {
    PortFailure::Infrastructure(InfrastructureFailure {
        code: code.to_string(),
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::implementation::{
        AcceptanceObligation, DispatchControls, DispatchWorkspace, IMPLEMENTATION_WORK_SCHEMA,
        ImplementationWork, SourceRef, TerminalPolicy, WorkBudget, WorkSourceKind,
        WorkspaceIsolation,
    };
    use kiln_core::types::{ContentId, ExternalRef, ProviderProfileId};
    use kiln_provider_api::WorkflowExchangeResponse;
    use lattice_workflow_contract::{
        ACCEPTED_SCHEMA, ContractLimits, ControlCapabilities, PROFILE_SCHEMA, REQUIRED_NON_CLAIMS,
        profile_identity, validate_request,
    };
    use lattice_workflow_contract::{
        EffectClass, GRAPH_SCHEMA, KilnLinkage, Label, ObservationPolicy, REQUEST_SCHEMA,
        RequestedControls, SourceContext, SourceValue, WorkflowEdge, WorkflowGraph, WorkflowNode,
        graph_identity, request_identity,
    };

    fn content(character: char) -> ContentId {
        ContentId::from_blake3_hex(character.to_string().repeat(64))
            .unwrap_or_else(|_| unreachable!())
    }
    fn identity(value: &ContentId) -> Identity {
        Identity::new(format!("b3:{}", value.as_str())).unwrap_or_else(|_| unreachable!())
    }
    fn work() -> ImplementationWork {
        ImplementationWork {
            schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
            revision: 1,
            actor: "operator".to_string(),
            repository: "onix/kiln".to_string(),
            project: "kiln".to_string(),
            base: Some(
                ExternalRef::new("git", "commit", "base").unwrap_or_else(|_| unreachable!()),
            ),
            title: "Lattice implementation".to_string(),
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: std::collections::BTreeSet::from(["write_source".to_string()]),
            verification_profile: content('9'),
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
                native_id: "onix/kiln#45".to_string(),
                version: content('1'),
            }],
        }
    }

    fn dispatch(attempt: &ImplementationAttempt) -> ImplementationDispatchPlan {
        ImplementationDispatchPlan {
            schema: kiln_core::implementation::DISPATCH_PLAN_SCHEMA.to_string(),
            work_identity: attempt.work_identity.clone(),
            work_revision: attempt.work_revision,
            attempt_identity: attempt.identity.clone(),
            fence: attempt.fence,
            repository: "onix/kiln".to_string(),
            base: attempt.base.clone(),
            workspace: DispatchWorkspace {
                identity: content('d'),
                isolation: WorkspaceIsolation::IsolatedSnapshot,
                network: false,
                credentials: false,
            },
            provider_profile: ProviderProfileId::new("lattice").unwrap_or_else(|_| unreachable!()),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: std::collections::BTreeSet::from(["write_source".to_string()]),
            controls: DispatchControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            deadline_minutes: 60,
            token_budget: 1000,
            invocation_budget: 10,
            expected_output: content('e'),
        }
    }

    fn request(
        attempt: &ImplementationAttempt,
        dispatch: &ImplementationDispatchPlan,
        reservation: &DispatchReservation,
    ) -> WorkflowRequest {
        let mut graph = WorkflowGraph {
            schema: GRAPH_SCHEMA.to_string(),
            identity: identity(&content('0')),
            nodes: vec![WorkflowNode {
                id: Label::new("impl".to_string()).unwrap_or_else(|_| unreachable!()),
                kind: Label::new("agent".to_string()).unwrap_or_else(|_| unreachable!()),
                effect_class: EffectClass::Custom,
                adapter_kind: Label::new("local".to_string()).unwrap_or_else(|_| unreachable!()),
                operation_identity: identity(&content('1')),
            }],
            edges: Vec::<WorkflowEdge>::new(),
        };
        graph.identity = graph_identity(&graph);
        let mut request = WorkflowRequest {
            schema: REQUEST_SCHEMA.to_string(),
            identity: identity(&content('0')),
            idempotency_key: identity(&reservation.plan),
            profile_identity: identity(&content('2')),
            platform: Label::new("linux-x86_64".to_string()).unwrap_or_else(|_| unreachable!()),
            kiln: KilnLinkage {
                effect_identity: identity(&reservation.plan),
                attempt_identity: identity(&attempt.identity),
                candidate_identity: identity(&dispatch.expected_output),
                pipeline_revision_identity: identity(&content('3')),
                workflow_policy_identity: identity(&content('4')),
            },
            source: Some(SourceContext {
                provider: Label::new("git".to_string()).unwrap_or_else(|_| unreachable!()),
                repository: SourceValue::new("onix/kiln".to_string())
                    .unwrap_or_else(|_| unreachable!()),
                revision: SourceValue::new("base".to_string()).unwrap_or_else(|_| unreachable!()),
            }),
            graph,
            controls: RequestedControls {
                cancel: true,
                pause: false,
                resume: false,
            },
            observation: ObservationPolicy {
                maximum_log_references: 1,
                maximum_log_read_bytes: 1024,
                maximum_total_log_bytes: 1024,
            },
        };
        request.identity = request_identity(&request);
        request
    }

    struct FakeTransport {
        accepted: WorkflowAccepted,
        disconnect: bool,
        operation: WorkflowExchangeOperation,
    }

    impl WorkflowExchangePort for FakeTransport {
        fn exchange(
            &mut self,
            request: WorkflowExchangeRequest,
        ) -> PortResult<WorkflowExchangeResponse> {
            assert_eq!(request.operation, self.operation);
            Ok(WorkflowExchangeResponse {
                body: serde_json::to_vec(&self.accepted).unwrap_or_else(|_| unreachable!()),
                log_bytes: Vec::new(),
                disconnected_after_acceptance: self.disconnect,
            })
        }
    }

    fn profile() -> lattice_workflow_contract::CapabilityProfile {
        lattice_workflow_contract::CapabilityProfile {
            schema: PROFILE_SCHEMA.to_string(),
            graph_schema: GRAPH_SCHEMA.to_string(),
            platforms: vec![
                Label::new("linux-x86_64".to_string()).unwrap_or_else(|_| unreachable!()),
            ],
            node_kinds: vec![Label::new("agent".to_string()).unwrap_or_else(|_| unreachable!())],
            effect_classes: vec![EffectClass::Custom],
            adapter_kinds: vec![Label::new("local".to_string()).unwrap_or_else(|_| unreachable!())],
            controls: ControlCapabilities {
                cancel: true,
                pause: false,
                resume: false,
            },
            limits: ContractLimits {
                maximum_nodes: 4,
                maximum_edges: 4,
                maximum_log_references: 4,
                maximum_log_read_bytes: 1024,
                maximum_total_log_bytes: 1024,
            },
            non_claims: REQUIRED_NON_CLAIMS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        }
    }

    #[test]
    // r[verify kiln.implementation.lattice_dispatch.scenario.admit]
    fn dispatches_an_accepted_request_and_keeps_disconnect_unknown() {
        let profile = profile();
        let request = request(
            &kiln_core::implementation::ImplementationAttempt {
                schema: kiln_core::implementation::ATTEMPT_SCHEMA.to_string(),
                identity: content('a'),
                canonical_key: content('b'),
                work_identity: content('c'),
                work_revision: 1,
                base: Some(
                    ExternalRef::new("git", "commit", "base").unwrap_or_else(|_| unreachable!()),
                ),
                ordinal: 1,
                fence: 4,
                lease_until_ms: 100,
                claimed_at_ms: 0,
                ended_at_ms: None,
                phase: kiln_core::implementation::AttemptPhase::Claimed,
                dispatch: None,
                workflow: None,
                candidate: None,
                handoff: None,
                verification: None,
                publication: None,
                metrics: None,
                failure: None,
                cause: None,
                superseded_by: None,
            },
            &ImplementationDispatchPlan {
                schema: kiln_core::implementation::DISPATCH_PLAN_SCHEMA.to_string(),
                work_identity: content('c'),
                work_revision: 1,
                attempt_identity: content('a'),
                fence: 4,
                repository: "onix/kiln".to_string(),
                base: Some(
                    ExternalRef::new("git", "commit", "base").unwrap_or_else(|_| unreachable!()),
                ),
                workspace: DispatchWorkspace {
                    identity: content('d'),
                    isolation: WorkspaceIsolation::IsolatedSnapshot,
                    network: false,
                    credentials: false,
                },
                provider_profile: ProviderProfileId::new("lattice")
                    .unwrap_or_else(|_| unreachable!()),
                permitted_paths: vec!["crates".to_string()],
                permitted_effects: std::collections::BTreeSet::from(["write_source".to_string()]),
                controls: DispatchControls {
                    cancel: true,
                    pause: false,
                    resume: false,
                },
                deadline_minutes: 60,
                token_budget: 1000,
                invocation_budget: 10,
                expected_output: content('e'),
            },
            &DispatchReservation {
                schema: kiln_core::implementation::DISPATCH_SCHEMA.to_string(),
                provider_profile: ProviderProfileId::new("lattice")
                    .unwrap_or_else(|_| unreachable!()),
                plan: content('f'),
                effect_id: kiln_core::types::EffectId::new("dispatch")
                    .unwrap_or_else(|_| unreachable!()),
                idempotency_key: ExternalRef::new("kiln", "implementation-dispatch", "id")
                    .unwrap_or_else(|_| unreachable!()),
                reserved_at_ms: 0,
            },
        );
        let mut request = request;
        request.profile_identity = profile_identity(&profile);
        request.identity = request_identity(&request);
        assert_eq!(validate_request(&profile, &request), Ok(()));
        let accepted = WorkflowAccepted {
            schema: ACCEPTED_SCHEMA.to_string(),
            request_identity: request.identity.clone(),
            idempotency_key: request.idempotency_key.clone(),
            workflow_identity: identity(&content('1')),
            execution_identity: identity(&content('2')),
        };
        let mut transport = FakeTransport {
            accepted: accepted.clone(),
            disconnect: false,
            operation: WorkflowExchangeOperation::Dispatch,
        };
        assert_eq!(
            dispatch_implementation_request(&mut transport, &profile, &request)
                .unwrap_or_else(|_| unreachable!()),
            accepted
        );
        let mut recovery = FakeTransport {
            accepted: accepted.clone(),
            disconnect: false,
            operation: WorkflowExchangeOperation::RecoverDispatch,
        };
        assert_eq!(
            recover_implementation_request(&mut recovery, &profile, &request)
                .unwrap_or_else(|_| unreachable!()),
            accepted
        );
        let mut disconnected = FakeTransport {
            accepted,
            disconnect: true,
            operation: WorkflowExchangeOperation::Dispatch,
        };
        assert!(dispatch_implementation_request(&mut disconnected, &profile, &request).is_err());
    }

    #[test]
    // r[verify kiln.implementation.lattice_dispatch] r[verify kiln.implementation.lattice_dispatch.scenario.admit] r[verify kiln.implementation.lattice_dispatch.scenario.refused]
    fn binds_an_exact_lattice_request_to_the_reserved_attempt() {
        let work = work();
        let mut ledger = kiln_core::implementation::AttemptLedger::default();
        let attempt = ledger
            .claim(&work, 0, kiln_core::implementation::DEFAULT_LEASE_MS)
            .unwrap_or_else(|_| unreachable!());
        let reservation = ledger
            .reserve_dispatch(
                &attempt.identity,
                attempt.fence,
                1,
                &content('f'),
                &ProviderProfileId::new("lattice").unwrap_or_else(|_| unreachable!()),
            )
            .unwrap_or_else(|_| unreachable!());
        let attempt = ledger
            .get(&attempt.identity)
            .cloned()
            .unwrap_or_else(|| unreachable!());
        let dispatch = dispatch(&attempt);
        let request = request(&attempt, &dispatch, &reservation);
        assert_eq!(request.identity, request_identity(&request));
        assert_eq!(request.graph.identity, graph_identity(&request.graph));
        let admitted = admit_implementation_request(&dispatch, &reservation, &attempt, &request);
        assert!(admitted.is_ok(), "{admitted:?}");
        let mut wrong = request.clone();
        wrong.kiln.attempt_identity = identity(&content('a'));
        assert!(admit_implementation_request(&dispatch, &reservation, &attempt, &wrong).is_err());

        let mut paused = request.clone();
        paused.controls.pause = true;
        paused.identity = request_identity(&paused);
        assert!(admit_implementation_request(&dispatch, &reservation, &attempt, &paused).is_err());

        let mut source = request.clone();
        source
            .source
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .repository =
            SourceValue::new("other/repo".to_string()).unwrap_or_else(|_| unreachable!());
        source.identity = request_identity(&source);
        assert!(admit_implementation_request(&dispatch, &reservation, &attempt, &source).is_err());

        let mut route = request;
        route
            .graph
            .nodes
            .first_mut()
            .unwrap_or_else(|| unreachable!())
            .kind = Label::new("build".to_string()).unwrap_or_else(|_| unreachable!());
        route.graph.identity = graph_identity(&route.graph);
        route.identity = request_identity(&route);
        assert!(admit_implementation_request(&dispatch, &reservation, &attempt, &route).is_err());
    }
}
