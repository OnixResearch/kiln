use super::*;
use crate::fleet::scope::parse_scope_text;
use alloc::vec;

const NOW: u64 = 1_800_000_000_000;
const COORDINATOR: &str = "britton-desktop";

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn scope(text: &str) -> FleetScope {
    parse_scope_text(text).unwrap_or_else(|_| unreachable!())
}

fn agent(tool: &str, host: &str, session: &str) -> FleetAgent {
    FleetAgent {
        tool: tool.to_string(),
        host: host.to_string(),
        session: session.to_string(),
    }
}

fn context<'a>(
    now_ms: u64,
    attempts: &'a [AttemptScope],
    foreign: &'a [ForeignClaim],
) -> ClaimContext<'a> {
    ClaimContext {
        now_ms,
        attempts,
        foreign,
        coordinator: COORDINATOR,
    }
}

fn acquire_command(text: &str, holder: &FleetAgent) -> ClaimCommand {
    ClaimCommand::Acquire {
        scope: scope(text),
        holder: holder.clone(),
        purpose: "fleet slice".to_string(),
        lease_ms: DEFAULT_CLAIM_LEASE_MS,
        attempt: None,
    }
}

fn granted(
    registry: &FleetClaimRegistry,
    command: ClaimCommand,
    context: &ClaimContext<'_>,
) -> (FleetClaimRegistry, FleetClaim, Vec<ClaimEffect>) {
    match decide(registry, command, context) {
        Ok((next, ClaimOutcome::Granted(claim), effects)) => (next, claim, effects),
        other => unreachable!("{other:?}"),
    }
}

fn outcome(
    registry: &FleetClaimRegistry,
    command: ClaimCommand,
    context: &ClaimContext<'_>,
) -> ClaimOutcome {
    decide(registry, command, context)
        .map(|(_, outcome, _)| outcome)
        .unwrap_or_else(|diagnostic| unreachable!("{diagnostic:?}"))
}

fn refused_code(
    registry: &FleetClaimRegistry,
    command: ClaimCommand,
    context: &ClaimContext<'_>,
) -> String {
    decide(registry, command, context)
        .err()
        .map(|diagnostic| diagnostic.code)
        .unwrap_or_else(|| unreachable!())
}

#[test]
// r[verify kiln.fleet.claims] r[verify kiln.fleet.claims.scenario.conflict]
fn conflicts_name_their_source() {
    let other = agent("codex", "aspen1", "s-other");
    let mine = agent("omp", "aspen3", "s-mine");
    let (registry, held, _) = granted(
        &FleetClaimRegistry::default(),
        acquire_command("OnixResearch/kiln crates", &other),
        &context(NOW, &[], &[]),
    );
    let attempts = [AttemptScope {
        attempt: content('a'),
        scope: scope("OnixResearch/kiln crates/kiln-fleet"),
    }];
    let foreign = [ForeignClaim {
        scope: scope("aspen3 system"),
        message_id: "<claim@aspen3>".to_string(),
        author: "aspen3 <aspen3@swarm.local>".to_string(),
        date_unix_ms: NOW - 1_000,
    }];
    let context = context(NOW, &attempts, &foreign);

    assert_eq!(
        outcome(
            &registry,
            acquire_command("OnixResearch/kiln crates/kiln-core", &mine),
            &context
        ),
        ClaimOutcome::Conflict(ClaimConflict::Claim {
            identity: held.identity.clone(),
            holder: "codex on aspen1, session s-other".to_string(),
            scope: "onixresearch/kiln crates".to_string(),
        })
    );
    // An attempt scope is outside the other holder's claim only when the
    // claim does not cover it, so check the attempt against a fresh registry.
    assert_eq!(
        outcome(
            &FleetClaimRegistry::default(),
            acquire_command("OnixResearch/kiln crates/kiln-fleet/src", &mine),
            &context
        ),
        ClaimOutcome::Conflict(ClaimConflict::Attempt {
            attempt: content('a'),
            scope: "onixresearch/kiln crates/kiln-fleet".to_string(),
        })
    );
    assert_eq!(
        outcome(
            &registry,
            acquire_command("aspen3 kiln.service", &mine),
            &context
        ),
        ClaimOutcome::Conflict(ClaimConflict::Foreign {
            message_id: "<claim@aspen3>".to_string(),
            author: "aspen3 <aspen3@swarm.local>".to_string(),
            scope: "aspen3 system".to_string(),
        })
    );
    let (_, docs, effects) = granted(
        &registry,
        acquire_command("OnixResearch/kiln docs", &mine),
        &context,
    );
    assert_eq!(docs.scope, scope("onixresearch/kiln docs"));
    assert_eq!(effects.len(), 1);
}

#[test]
// r[verify kiln.fleet.claims]
fn holders_attempts_and_stale_sources_do_not_conflict() {
    let mine = agent("omp", "aspen3", "s-mine");
    let other = agent("codex", "aspen1", "s-other");
    let (registry, first, _) = granted(
        &FleetClaimRegistry::default(),
        acquire_command("OnixResearch/kiln crates", &mine),
        &context(NOW, &[], &[]),
    );
    // Identical scope from the same holder is idempotent.
    assert_eq!(
        decide(
            &registry,
            acquire_command("onixresearch/kiln crates/", &mine),
            &context(NOW + 1, &[], &[])
        ),
        Ok((
            registry.clone(),
            ClaimOutcome::Existing(first.clone()),
            Vec::new()
        ))
    );
    // A different overlapping scope from the same holder is granted.
    let (registry, nested, _) = granted(
        &registry,
        acquire_command("OnixResearch/kiln crates/kiln-core", &mine),
        &context(NOW + 2, &[], &[]),
    );
    assert!(nested.fence > first.fence);
    assert_ne!(nested.identity, first.identity);
    // The same session on another tool is another holder.
    assert!(matches!(
        outcome(
            &registry,
            acquire_command(
                "OnixResearch/kiln crates",
                &agent("claude", "aspen3", "s-mine")
            ),
            &context(NOW + 3, &[], &[])
        ),
        ClaimOutcome::Conflict(ClaimConflict::Claim { .. })
    ));
    // Expired claims no longer conflict, even before a sweep.
    let after = first.expires_ms;
    assert!(matches!(
        decide(
            &registry,
            acquire_command("OnixResearch/kiln crates", &other),
            &context(after + DEFAULT_CLAIM_LEASE_MS, &[], &[])
        ),
        Ok((_, ClaimOutcome::Granted(_), _))
    ));
    // The attempt's own claim does not conflict with that attempt.
    let attempts = [AttemptScope {
        attempt: content('a'),
        scope: scope("OnixResearch/kiln src"),
    }];
    let command = ClaimCommand::Acquire {
        scope: scope("OnixResearch/kiln src/lib.rs"),
        holder: other.clone(),
        purpose: "attempt work".to_string(),
        lease_ms: DEFAULT_CLAIM_LEASE_MS,
        attempt: Some(content('a')),
    };
    let (_, own, _) = granted(&registry, command, &context(NOW, &attempts, &[]));
    assert_eq!(own.attempt, Some(content('a')));
    // Foreign claims older than the stale window do not conflict.
    let foreign = [ForeignClaim {
        scope: scope("aspen3 system"),
        message_id: "<old@aspen3>".to_string(),
        author: "aspen3".to_string(),
        date_unix_ms: NOW - FOREIGN_CLAIM_STALE_MS - 1,
    }];
    assert!(matches!(
        decide(
            &registry,
            acquire_command("aspen3 system", &other),
            &context(NOW, &[], &foreign)
        ),
        Ok((_, ClaimOutcome::Granted(_), _))
    ));
}

#[test]
// r[verify kiln.fleet.claims]
fn malformed_acquisitions_are_refused() {
    let registry = FleetClaimRegistry::default();
    let context = context(NOW, &[], &[]);
    let base = |holder: FleetAgent, purpose: &str, lease_ms: u64| ClaimCommand::Acquire {
        scope: scope("OnixResearch/kiln docs"),
        holder,
        purpose: purpose.to_string(),
        lease_ms,
        attempt: None,
    };
    let holder = agent("omp", "aspen3", "s1");
    assert_eq!(
        refused_code(
            &registry,
            base(agent("omp", "aspen 3", "s1"), "x", DEFAULT_CLAIM_LEASE_MS),
            &context
        ),
        "fleet_claim_holder"
    );
    assert_eq!(
        refused_code(
            &registry,
            base(agent("", "aspen3", "s1"), "x", DEFAULT_CLAIM_LEASE_MS),
            &context
        ),
        "fleet_claim_holder"
    );
    assert_eq!(
        refused_code(
            &registry,
            base(holder.clone(), "", DEFAULT_CLAIM_LEASE_MS),
            &context
        ),
        "fleet_claim_purpose"
    );
    assert_eq!(
        refused_code(
            &registry,
            base(
                holder.clone(),
                &"p".repeat(MAX_PURPOSE_BYTES + 1),
                DEFAULT_CLAIM_LEASE_MS
            ),
            &context
        ),
        "fleet_claim_purpose"
    );
    assert_eq!(
        refused_code(
            &registry,
            base(holder.clone(), "a\nb", DEFAULT_CLAIM_LEASE_MS),
            &context
        ),
        "fleet_claim_purpose"
    );
    assert_eq!(
        refused_code(
            &registry,
            base(holder.clone(), "x", MIN_CLAIM_LEASE_MS - 1),
            &context
        ),
        "fleet_claim_lease"
    );
    assert_eq!(
        refused_code(
            &registry,
            base(holder.clone(), "x", MAX_CLAIM_LEASE_MS + 1),
            &context
        ),
        "fleet_claim_lease"
    );
    // A secret-shaped purpose cannot be mirrored, so the claim is refused.
    assert_eq!(
        refused_code(
            &registry,
            base(holder.clone(), "token ghp_abc", DEFAULT_CLAIM_LEASE_MS),
            &context
        ),
        "fleet_claim_mirror"
    );
    let uncanonical = ClaimCommand::Acquire {
        scope: FleetScope {
            target: crate::fleet::FleetTarget::Repository("OnixResearch/kiln".to_string()),
            path: crate::fleet::ScopePath::Whole,
        },
        holder,
        purpose: "x".to_string(),
        lease_ms: DEFAULT_CLAIM_LEASE_MS,
        attempt: None,
    };
    assert_eq!(
        refused_code(&registry, uncanonical, &context),
        "fleet_scope"
    );
}

#[test]
// r[verify kiln.fleet.claims]
fn open_claims_are_bounded() {
    let mut registry = FleetClaimRegistry::default();
    for index in 0..MAX_OPEN_FLEET_CLAIMS {
        let holder = agent("omp", "aspen3", &format!("s{index}"));
        let (next, _, _) = granted(
            &registry,
            acquire_command(&format!("OnixResearch/kiln path{index}"), &holder),
            &context(NOW, &[], &[]),
        );
        registry = next;
    }
    assert_eq!(
        refused_code(
            &registry,
            acquire_command("OnixResearch/kiln last", &agent("omp", "aspen3", "last")),
            &context(NOW, &[], &[])
        ),
        "fleet_claim_limit"
    );
}

#[test]
// r[verify kiln.fleet.claims] r[verify kiln.fleet.claims.scenario.lease]
fn stale_fences_are_refused_and_sweeps_expire() {
    let holder = agent("omp", "aspen3", "s1");
    let (registry, claim, _) = granted(
        &FleetClaimRegistry::default(),
        acquire_command("OnixResearch/kiln docs", &holder),
        &context(NOW, &[], &[]),
    );
    let ctx = context(NOW + 1_000, &[], &[]);
    let stale_renew = ClaimCommand::Renew {
        identity: claim.identity.clone(),
        fence: claim.fence - 1,
        lease_ms: DEFAULT_CLAIM_LEASE_MS,
    };
    assert_eq!(
        refused_code(&registry, stale_renew, &ctx),
        "fleet_claim_stale"
    );
    let stale_release = ClaimCommand::Release {
        identity: claim.identity.clone(),
        fence: claim.fence + 1,
        note: String::new(),
    };
    assert_eq!(
        refused_code(&registry, stale_release, &ctx),
        "fleet_claim_stale"
    );
    assert_eq!(
        refused_code(
            &registry,
            ClaimCommand::Renew {
                identity: content('f'),
                fence: claim.fence,
                lease_ms: DEFAULT_CLAIM_LEASE_MS,
            },
            &ctx
        ),
        "fleet_claim_unknown"
    );
    assert_eq!(
        refused_code(
            &registry,
            ClaimCommand::Renew {
                identity: claim.identity.clone(),
                fence: claim.fence,
                lease_ms: MAX_CLAIM_LEASE_MS + 1,
            },
            &ctx
        ),
        "fleet_claim_lease"
    );

    let renewed = decide(
        &registry,
        ClaimCommand::Renew {
            identity: claim.identity.clone(),
            fence: claim.fence,
            lease_ms: MAX_CLAIM_LEASE_MS,
        },
        &ctx,
    );
    let Ok((registry, ClaimOutcome::Renewed(renewed), effects)) = renewed else {
        unreachable!()
    };
    assert_eq!(renewed.expires_ms, NOW + 1_000 + MAX_CLAIM_LEASE_MS);
    assert!(effects.is_empty());

    // Before expiry a sweep changes nothing.
    let (unchanged, swept, effects) =
        decide(&registry, ClaimCommand::Sweep, &ctx).unwrap_or_else(|_| unreachable!());
    assert_eq!(unchanged, registry);
    assert_eq!(swept, ClaimOutcome::Swept(Vec::new()));
    assert!(effects.is_empty());

    let late = context(renewed.expires_ms, &[], &[]);
    assert_eq!(
        refused_code(
            &registry,
            ClaimCommand::Renew {
                identity: claim.identity.clone(),
                fence: claim.fence,
                lease_ms: DEFAULT_CLAIM_LEASE_MS,
            },
            &late
        ),
        "fleet_claim_stale"
    );
    let (registry, swept, effects) =
        decide(&registry, ClaimCommand::Sweep, &late).unwrap_or_else(|_| unreachable!());
    assert_eq!(swept, ClaimOutcome::Swept(vec![claim.identity.clone()]));
    let expired = registry
        .claims
        .get(&claim.identity)
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        expired.state,
        ClaimState::Expired {
            at_ms: renewed.expires_ms
        }
    );
    assert!(matches!(
        effects.as_slice(),
        [ClaimEffect::PostRelease { subject, body, .. }]
            if subject == "RELEASE onixresearch/kiln docs" && body.contains(EXPIRED_RELEASE_REASON)
    ));
    // A second sweep requests nothing more, and the expired claim is closed.
    let (_, swept, effects) =
        decide(&registry, ClaimCommand::Sweep, &late).unwrap_or_else(|_| unreachable!());
    assert_eq!(swept, ClaimOutcome::Swept(Vec::new()));
    assert!(effects.is_empty());
    assert_eq!(
        refused_code(
            &registry,
            ClaimCommand::Release {
                identity: claim.identity,
                fence: claim.fence,
                note: String::new(),
            },
            &late
        ),
        "fleet_claim_stale"
    );
}

#[test]
// r[verify kiln.fleet.mirror] r[verify kiln.fleet.mirror.scenario.articles]
fn claim_and_release_articles_reference_the_claim() {
    let holder = agent("claude", "aspen3", "0b8d3f5e-6c1a-4c2e-9d7f-2f6e1b0a9c11");
    let (registry, claim, effects) = granted(
        &FleetClaimRegistry::default(),
        acquire_command("OnixResearch/kiln docs", &holder),
        &context(NOW, &[], &[]),
    );
    assert_eq!(claim.claim_mirror, MirrorStatus::Pending);
    let [
        ClaimEffect::PostClaim {
            identity,
            subject,
            body,
        },
    ] = effects.as_slice()
    else {
        unreachable!()
    };
    assert_eq!(identity, &claim.identity);
    assert_eq!(subject, "CLAIM onixresearch/kiln docs");
    assert!(body.contains("purpose: fleet slice"));
    assert!(body.contains("expected lease: 240 minutes"));
    assert!(body.contains("— kiln on britton-desktop for claude on aspen3, session 0b8d3f5e-6c1"));
    assert_eq!(
        pending_mirror_effects(&registry, COORDINATOR),
        Ok(effects.clone())
    );

    let ctx = context(NOW + 1, &[], &[]);
    let posted = MirrorStatus::Posted {
        message_id: Some("<claim@kiln>".to_string()),
    };
    let (registry, recorded, effects) = decide(
        &registry,
        ClaimCommand::MirrorObserved {
            identity: claim.identity.clone(),
            release: false,
            status: posted.clone(),
        },
        &ctx,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(recorded, ClaimOutcome::Recorded);
    assert!(effects.is_empty());
    assert_eq!(
        pending_mirror_effects(&registry, COORDINATOR),
        Ok(Vec::new())
    );
    // A settled post cannot be rewritten; repeating it is idempotent.
    assert_eq!(
        refused_code(
            &registry,
            ClaimCommand::MirrorObserved {
                identity: claim.identity.clone(),
                release: false,
                status: MirrorStatus::Failed,
            },
            &ctx
        ),
        "fleet_claim_mirror"
    );
    assert!(matches!(
        decide(
            &registry,
            ClaimCommand::MirrorObserved {
                identity: claim.identity.clone(),
                release: false,
                status: posted,
            },
            &ctx
        ),
        Ok((_, ClaimOutcome::Recorded, _))
    ));
    // No release article was requested while the claim is open.
    assert_eq!(
        refused_code(
            &registry,
            ClaimCommand::MirrorObserved {
                identity: claim.identity.clone(),
                release: true,
                status: MirrorStatus::Unknown,
            },
            &ctx
        ),
        "fleet_claim_mirror"
    );

    let (registry, released, effects) = decide(
        &registry,
        ClaimCommand::Release {
            identity: claim.identity.clone(),
            fence: claim.fence,
            note: "docs landed".to_string(),
        },
        &ctx,
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(matches!(released, ClaimOutcome::Released(_)));
    let [
        ClaimEffect::PostRelease {
            subject,
            references,
            body,
            ..
        },
    ] = effects.as_slice()
    else {
        unreachable!()
    };
    assert_eq!(subject, "RELEASE onixresearch/kiln docs");
    assert_eq!(references.as_deref(), Some("<claim@kiln>"));
    assert!(body.contains("docs landed"));

    // An unknown release post is reconciled, never re-requested.
    let (registry, _, _) = decide(
        &registry,
        ClaimCommand::MirrorObserved {
            identity: claim.identity.clone(),
            release: true,
            status: MirrorStatus::Unknown,
        },
        &ctx,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        pending_mirror_effects(&registry, COORDINATOR),
        Ok(Vec::new())
    );
    let (registry, _, _) = decide(
        &registry,
        ClaimCommand::MirrorObserved {
            identity: claim.identity.clone(),
            release: true,
            status: MirrorStatus::Posted { message_id: None },
        },
        &ctx,
    )
    .unwrap_or_else(|_| unreachable!());
    let (registry, _, _) = decide(
        &registry,
        ClaimCommand::MirrorObserved {
            identity: claim.identity.clone(),
            release: true,
            status: MirrorStatus::Posted {
                message_id: Some("<release@kiln>".to_string()),
            },
        },
        &ctx,
    )
    .unwrap_or_else(|_| unreachable!());
    assert_eq!(
        registry
            .claims
            .get(&claim.identity)
            .map(|claim| claim.release_mirror.clone()),
        Some(MirrorStatus::Posted {
            message_id: Some("<release@kiln>".to_string())
        })
    );
    for status in [
        MirrorStatus::Pending,
        MirrorStatus::Posted {
            message_id: Some("bad id".to_string()),
        },
    ] {
        assert_eq!(
            refused_code(
                &registry,
                ClaimCommand::MirrorObserved {
                    identity: claim.identity.clone(),
                    release: true,
                    status,
                },
                &ctx
            ),
            "fleet_claim_mirror"
        );
    }
}

#[test]
// r[verify kiln.fleet.mirror]
fn release_without_a_known_claim_id_has_no_references() {
    let holder = agent("omp", "aspen3", "s1");
    let (registry, claim, _) = granted(
        &FleetClaimRegistry::default(),
        acquire_command("aspen3 /var/lib/kiln", &holder),
        &context(NOW, &[], &[]),
    );
    let (registry, _, effects) = decide(
        &registry,
        ClaimCommand::Release {
            identity: claim.identity.clone(),
            fence: claim.fence,
            note: String::new(),
        },
        &context(NOW + 5, &[], &[]),
    )
    .unwrap_or_else(|_| unreachable!());
    assert!(matches!(
        effects.as_slice(),
        [ClaimEffect::PostRelease { references: None, body, .. }]
            if body.contains(DEFAULT_RELEASE_REASON)
    ));
    // Both the claim and release articles are still owed after a crash.
    assert_eq!(
        pending_mirror_effects(&registry, COORDINATOR).map(|effects| effects.len()),
        Ok(2)
    );
}

#[test]
// r[verify kiln.fleet.claims]
fn identities_are_deterministic_and_registry_round_trips() {
    let holder = agent("omp", "aspen3", "s1");
    let docs = scope("OnixResearch/kiln docs");
    let first = claim_identity(&docs, &holder, NOW, 1).unwrap_or_else(|_| unreachable!());
    assert_eq!(claim_identity(&docs, &holder, NOW, 1), Ok(first.clone()));
    assert_ne!(claim_identity(&docs, &holder, NOW, 2), Ok(first.clone()));
    assert_ne!(
        claim_identity(&docs, &holder, NOW + 1, 1),
        Ok(first.clone())
    );
    assert_ne!(
        claim_identity(&docs, &agent("omp", "aspen3", "s2"), NOW, 1),
        Ok(first)
    );
    let (registry, claim, _) = granted(
        &FleetClaimRegistry::default(),
        acquire_command("OnixResearch/kiln docs", &holder),
        &context(NOW, &[], &[]),
    );
    assert_eq!(claim.fence, FIRST_FLEET_FENCE);
    assert_eq!(registry.next_fence, FIRST_FLEET_FENCE + 1);
    let encoded = serde_json::to_string(&registry).unwrap_or_else(|_| unreachable!());
    let decoded: FleetClaimRegistry =
        serde_json::from_str(&encoded).unwrap_or_else(|_| unreachable!());
    assert_eq!(decoded, registry);
}
