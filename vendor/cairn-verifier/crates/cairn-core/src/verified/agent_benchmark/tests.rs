use super::*;

fn id(c: char) -> String {
    c.to_string().repeat(64)
}
fn policy() -> BenchmarkPolicy {
    BenchmarkPolicy::bundled().unwrap()
}
fn cohort(product: char) -> Cohort {
    Cohort {
        source_projection: id('a'),
        product_revision: id(product),
        policy: id('c'),
        fixture: id('d'),
        oracle: id('e'),
        model: id('f'),
        agent_configuration: id('1'),
        catalog: id('2'),
        presentation: id('3'),
        accounting: Accounting::ProviderUsage {
            usage_source: id('4'),
        },
    }
}
fn usage(
    uncached: u64,
    cached: u64,
    output: u64,
    reasoning: u64,
    inclusion: ReasoningInclusion,
) -> TokenUsage {
    TokenUsage {
        uncached_input: uncached,
        cached_input: cached,
        output,
        reasoning: Some(reasoning),
        reasoning_inclusion: inclusion,
    }
}
fn run(
    product: char,
    name: char,
    verdict: OracleVerdict,
    claimed: bool,
    usage: Option<TokenUsage>,
) -> Run {
    Run {
        run_id: id(name),
        cohort: cohort(product),
        trace: Trace {
            declared_calls: 1,
            complete: true,
            calls: vec![Call {
                call_id: id(name),
                sequence: 0,
                usage,
                content_identity: None,
            }],
            retry_count: 0,
            tool_call_count: 2,
            repeated_read_count: 1,
            duration_ms: 42,
        },
        oracle: OracleObservation {
            verdict,
            agent_claimed_completion: claimed,
        },
    }
}
fn input(runs: Vec<Run>) -> BenchmarkInput {
    BenchmarkInput {
        schema: INPUT_SCHEMA.into(),
        scenario: Scenario {
            scope: "fresh-authoring".into(),
            mode: ObservationMode::LiveHost,
            repetitions: runs.len(),
            max_calls_per_run: 4,
        },
        runs,
    }
}
fn pair(base: BenchmarkInput, candidate: BenchmarkInput) -> ComparisonInput {
    ComparisonInput {
        schema: COMPARISON_SCHEMA.into(),
        allowed_candidate_differences: vec![AllowedDifference::ProductRevision],
        baseline: base,
        candidate,
    }
}
fn standard() -> TokenUsage {
    usage(5, 3, 7, 2, ReasoningInclusion::IncludedInOutput)
}

#[test]
fn exact_accounting_avoids_reasoning_double_charge_and_keeps_cached_input() {
    let mut observed = run('b', 'a', OracleVerdict::Completed, false, Some(standard()));
    observed.trace.calls.push(Call {
        call_id: id('f'),
        sequence: 1,
        usage: Some(usage(1, 4, 6, 2, ReasoningInclusion::IncludedInOutput)),
        content_identity: None,
    });
    observed.trace.declared_calls = 2;
    let report = validate(&policy(), &input(vec![observed])).unwrap();
    assert_eq!(
        (
            report.totals.uncached_input,
            report.totals.cached_input,
            report.totals.output,
            report.totals.reasoning
        ),
        (6, 7, 13, 4)
    );
    assert_eq!(report.totals.billed_total, 26);
    assert_eq!(
        report
            .tokens_per_correct_completion
            .unwrap()
            .attempted_tokens,
        26
    );
    assert_eq!(
        (
            report.totals.tool_call_count,
            report.totals.repeated_read_count,
            report.totals.duration_ms
        ),
        (2, 1, 42)
    );
}
#[test]
fn reasoning_additional_is_charged_once() {
    let report = validate(
        &policy(),
        &input(vec![run(
            'b',
            'a',
            OracleVerdict::Completed,
            true,
            Some(usage(5, 3, 7, 2, ReasoningInclusion::AdditionalToOutput)),
        )]),
    )
    .unwrap();
    assert_eq!(report.totals.billed_output, 9);
    assert_eq!(report.totals.billed_total, 17);
}
#[test]
fn false_completion_and_failure_costs_remain_in_denominator() {
    let runs = vec![
        run('b', 'a', OracleVerdict::Completed, false, Some(standard())),
        run('b', 'b', OracleVerdict::Rejected, true, Some(standard())),
        run('b', 'c', OracleVerdict::Rejected, false, Some(standard())),
        run(
            'b',
            'd',
            OracleVerdict::CorrectlyBlocked,
            false,
            Some(standard()),
        ),
        run(
            'b',
            'e',
            OracleVerdict::Inconclusive,
            true,
            Some(standard()),
        ),
    ];
    let report = validate(&policy(), &input(runs)).unwrap();
    assert_eq!(
        (
            report.outcomes.completed,
            report.outcomes.correctly_blocked,
            report.outcomes.false_completion,
            report.outcomes.failed,
            report.outcomes.incomplete
        ),
        (1, 1, 1, 1, 1)
    );
    assert_eq!(report.totals.billed_total, 75);
    assert_eq!(
        report
            .tokens_per_correct_completion
            .unwrap()
            .attempted_tokens,
        75
    );
    assert_eq!(report.tokens_per_correct_block.unwrap().correct_blocks, 1);
}
#[test]
fn absent_usage_is_unavailable_not_estimated_or_dropped() {
    let report = validate(
        &policy(),
        &input(vec![
            run('b', 'a', OracleVerdict::Completed, true, Some(standard())),
            run('b', 'b', OracleVerdict::Rejected, true, None),
        ]),
    )
    .unwrap();
    assert_eq!(report.totals.billed_total, 15);
    assert!(!report.totals.complete_cost);
    assert_eq!(report.outcomes.false_completion, 1);
    assert!(report.tokens_per_correct_completion.is_none());
    assert!(!report.runs[1].cost_available);
}
#[test]
fn incomplete_and_duplicated_traces_fail_closed() {
    let mut truncated = input(vec![run(
        'b',
        'a',
        OracleVerdict::Inconclusive,
        false,
        Some(standard()),
    )]);
    truncated.runs[0].trace.declared_calls = 2;
    assert_eq!(
        validate(&policy(), &truncated).unwrap_err().class,
        "trace.incomplete"
    );
    truncated.runs[0].trace.declared_calls = 1;
    truncated.runs[0].trace.complete = false;
    assert_eq!(
        validate(&policy(), &truncated).unwrap_err().class,
        "trace.incomplete"
    );
    let mut repeated = input(vec![
        run('b', 'a', OracleVerdict::Completed, true, Some(standard())),
        run('b', 'b', OracleVerdict::Completed, true, Some(standard())),
    ]);
    repeated.runs[1].trace.calls[0].call_id = repeated.runs[0].trace.calls[0].call_id.clone();
    assert_eq!(
        validate(&policy(), &repeated).unwrap_err().class,
        "trace.duplicate_call"
    );
}
#[test]
fn impossible_reasoning_and_checked_overflow_are_rejected() {
    let impossible = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(usage(0, 0, 1, 2, ReasoningInclusion::IncludedInOutput)),
    )]);
    assert_eq!(
        validate(&policy(), &impossible).unwrap_err().class,
        "accounting.invalid"
    );
    let limit = 1_000_000_000_000_000_000;
    let mut overflow = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]);
    overflow.scenario.max_calls_per_run = 8;
    overflow.runs[0].trace.calls = (0..7)
        .map(|index| Call {
            call_id: format!("{index:064x}"),
            sequence: index,
            usage: Some(usage(
                limit,
                limit,
                limit,
                0,
                ReasoningInclusion::IncludedInOutput,
            )),
            content_identity: None,
        })
        .collect();
    overflow.runs[0].trace.declared_calls = 7;
    assert_eq!(
        validate(&policy(), &overflow).unwrap_err().class,
        "accounting.overflow"
    );
}
#[test]
fn tokenizer_requires_immutable_exposed_content_identity() {
    let mut observed = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]);
    observed.runs[0].cohort.accounting = Accounting::ExactTokenizer {
        implementation_revision: id('5'),
        encoding: id('6'),
    };
    assert_eq!(
        validate(&policy(), &observed).unwrap_err().class,
        "accounting.invalid"
    );
    observed.runs[0].trace.calls[0].content_identity = Some(id('7'));
    assert_eq!(
        validate(&policy(), &observed).unwrap().totals.billed_total,
        15
    );
}
#[test]
fn unknown_and_duplicate_fields_cannot_be_silently_accepted() {
    let json = serde_json::to_string(&input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]))
    .unwrap();
    let unknown = json.replacen("\"sequence\":0", "\"sequence\":0,\"prompt\":\"secret\"", 1);
    assert!(serde_json::from_str::<BenchmarkInput>(&unknown).is_err());
    let duplicate = json.replacen("\"sequence\":0", "\"sequence\":0,\"sequence\":0", 1);
    assert!(serde_json::from_str::<BenchmarkInput>(&duplicate).is_err());
    assert!(
        serde_json::from_str::<BenchmarkInput>(&json.replace("\"completed\"", "\"invented\""))
            .is_err()
    );
}
#[test]
fn comparison_rejects_drift_and_never_claims_replay_as_live() {
    let baseline = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]);
    let mut candidate = input(vec![run(
        '9',
        'b',
        OracleVerdict::Completed,
        true,
        Some(usage(1, 0, 1, 0, ReasoningInclusion::IncludedInOutput)),
    )]);
    candidate.runs[0].cohort.oracle = id('8');
    let drifted = compare(&policy(), &pair(baseline.clone(), candidate.clone())).unwrap();
    assert!(!drifted.comparable);
    assert!(drifted.drift_fields.contains(&"oracle".into()));
    assert_eq!(drifted.verdict, Verdict::Withheld);
    candidate.runs[0].cohort.oracle = id('e');
    let live = compare(&policy(), &pair(baseline.clone(), candidate.clone())).unwrap();
    assert_eq!(live.verdict, Verdict::Improved);
    assert_eq!(
        live.scoped_token_change.unwrap().candidate_attempted_tokens,
        2
    );
    let mut replay = pair(baseline, candidate);
    replay.baseline.scenario.mode = ObservationMode::RepresentationReplay;
    replay.candidate.scenario.mode = ObservationMode::RepresentationReplay;
    let replay_report = compare(&policy(), &replay).unwrap();
    assert_eq!(replay_report.verdict, Verdict::Withheld);
    assert!(replay_report
        .withheld_reasons
        .contains(&"representation_only".into()));
}
#[test]
fn correctness_regression_and_zero_denominator_withhold_improvement() {
    let base = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]);
    let candidate = input(vec![run(
        '9',
        'b',
        OracleVerdict::Rejected,
        true,
        Some(usage(1, 0, 0, 0, ReasoningInclusion::IncludedInOutput)),
    )]);
    let report = compare(&policy(), &pair(base, candidate)).unwrap();
    assert_eq!(report.verdict, Verdict::Withheld);
    assert!(report
        .withheld_reasons
        .contains(&"correctness_regression".into()));
    assert!(report.scoped_token_change.is_none());
}
#[test]
fn policy_bounds_and_run_cohort_are_enforced() {
    let mut sample = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]);
    sample.scenario.repetitions = 2;
    assert_eq!(
        validate(&policy(), &sample).unwrap_err().class,
        "input.bounds"
    );
    sample.runs.push(run(
        '9',
        'b',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    ));
    assert_eq!(
        validate(&policy(), &sample).unwrap_err().class,
        "input.identity"
    );
    sample.runs[1].cohort = cohort('b');
    sample.runs[1].trace.calls[0].sequence = 1;
    assert_eq!(
        validate(&policy(), &sample).unwrap_err().class,
        "trace.incomplete"
    );
}

#[test]
fn missing_reasoning_is_not_silently_zero_or_estimated() {
    let mut inclusive = standard();
    inclusive.reasoning = None;
    let report = validate(
        &policy(),
        &input(vec![run(
            'b',
            'a',
            OracleVerdict::Completed,
            true,
            Some(inclusive),
        )]),
    )
    .unwrap();
    assert_eq!(report.totals.billed_total, 15);
    assert!(report.totals.complete_cost);
    assert!(!report.totals.reasoning_complete);
    let mut additional = usage(5, 3, 7, 2, ReasoningInclusion::AdditionalToOutput);
    additional.reasoning = None;
    let report = validate(
        &policy(),
        &input(vec![run(
            'b',
            'a',
            OracleVerdict::Completed,
            true,
            Some(additional),
        )]),
    )
    .unwrap();
    assert!(!report.totals.complete_cost);
    assert!(report.tokens_per_correct_completion.is_none());
}

#[test]
fn mixed_conventions_and_policy_drift_cannot_claim_savings() {
    let mut mixed = input(vec![
        run('b', 'a', OracleVerdict::Completed, true, Some(standard())),
        run(
            'b',
            'b',
            OracleVerdict::Completed,
            true,
            Some(usage(1, 0, 1, 0, ReasoningInclusion::AdditionalToOutput)),
        ),
    ]);
    assert_eq!(
        validate(&policy(), &mixed).unwrap_err().class,
        "accounting.invalid"
    );
    mixed.runs[1].trace.calls[0]
        .usage
        .as_mut()
        .unwrap()
        .reasoning_inclusion = ReasoningInclusion::IncludedInOutput;
    let mut candidate = input(vec![run(
        '9',
        'c',
        OracleVerdict::Completed,
        true,
        Some(usage(1, 0, 1, 0, ReasoningInclusion::IncludedInOutput)),
    )]);
    candidate.runs[0].cohort.policy = id('8');
    let report = compare(
        &policy(),
        &pair(input(vec![mixed.runs.remove(0)]), candidate),
    )
    .unwrap();
    assert_eq!(report.verdict, Verdict::Withheld);
    assert!(report.drift_fields.contains(&"policy".into()));
    assert!(report.scoped_token_change.is_none());
}

#[test]
fn empty_scope_and_duplicate_run_ids_fail_closed() {
    let mut sample = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]);
    sample.scenario.scope.clear();
    assert_eq!(
        validate(&policy(), &sample).unwrap_err().class,
        "input.bounds"
    );
    sample.scenario.scope = "fresh-authoring".into();
    sample.runs.push(run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    ));
    sample.scenario.repetitions = 2;
    assert_eq!(
        validate(&policy(), &sample).unwrap_err().class,
        "input.identity"
    );
}

#[test]
fn changed_run_budget_cannot_be_hidden_as_a_product_difference() {
    let baseline = input(vec![run(
        'b',
        'a',
        OracleVerdict::Completed,
        true,
        Some(standard()),
    )]);
    let mut candidate = baseline.clone();
    candidate.runs[0].cohort.product_revision = id('c');
    candidate.scenario.max_calls_per_run += 1;
    let report = compare(&policy(), &pair(baseline, candidate)).unwrap();
    assert!(!report.comparable);
    assert_eq!(report.drift_fields, vec!["max_calls_per_run"]);
    assert_eq!(report.verdict, Verdict::Withheld);
    assert!(report.scoped_token_change.is_none());
}
