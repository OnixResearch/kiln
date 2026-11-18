//! Read-only recovery of a landed stack, and layer gate states.
use super::tests::{commit_body, serve, token};
use super::*;
use kiln_core::implementation::{
    LayerGate, STACK_PUBLICATION_SCHEMA, STACK_SCHEMA, StackLayerPlan, layer_branch, stack_gate,
    stack_identity,
};

const BASE: char = 'b';
const LOWER: char = '1';
const UPPER: char = '2';
const LOWER_TREE: char = 'e';
const UPPER_TREE: char = 'f';
const LOWER_MERGE: char = 'c';
const UPPER_MERGE: char = 'd';
/// A commit outside the stack: a rewritten head or an advanced trunk.
const OTHER: char = '9';

type Response = (&'static str, String);

fn git(kind: &str, character: char) -> ExternalRef {
    ExternalRef::new("git", kind, &character.to_string().repeat(40))
        .unwrap_or_else(|_| unreachable!())
}

fn github(kind: &str, number: u64) -> ExternalRef {
    ExternalRef::new("github", kind, &format!("octocat/hello-world#{number}"))
        .unwrap_or_else(|_| unreachable!())
}

fn content(character: char) -> ContentId {
    ContentId::from_blake3_hex(character.to_string().repeat(64)).unwrap_or_else(|_| unreachable!())
}

fn stack() -> (StackPlan, StackPublication) {
    let work = content('1');
    let attempt = content('2');
    let layer = |position: u32, candidate: char, tree: char, parent: char, base_branch: String| {
        StackLayerPlan {
            position,
            candidate: git("commit", candidate),
            tree: git("tree", tree),
            parent: git("commit", parent),
            verification: content('5'),
            branch: layer_branch(&work, &attempt, position),
            base_branch,
            title: format!("Layer {position}"),
            body: "Verified layer".to_string(),
        }
    };
    let plan = StackPlan {
        schema: STACK_SCHEMA.to_string(),
        work_identity: work.clone(),
        work_revision: 1,
        attempt: attempt.clone(),
        fence: 3,
        handoff: content('3'),
        verification: content('4'),
        repository: "octocat/hello-world".to_string(),
        base: git("commit", BASE),
        trunk_branch: "main".to_string(),
        layers: vec![
            layer(1, LOWER, LOWER_TREE, BASE, "main".to_string()),
            layer(
                2,
                UPPER,
                UPPER_TREE,
                LOWER,
                layer_branch(&work, &attempt, 1),
            ),
        ],
    };
    let publication = StackPublication {
        schema: STACK_PUBLICATION_SCHEMA.to_string(),
        stack: stack_identity(&plan).unwrap_or_else(|_| unreachable!()),
        provider_stack: github("stack", 5),
        pull_requests: vec![github("pull_request", 11), github("pull_request", 12)],
    };
    (plan, publication)
}

fn pull(number: u64, head: char, merge: Option<char>) -> Response {
    let body = serde_json::json!({
        "number": number,
        "user": {"login": "author"},
        "state": if merge.is_some() { "closed" } else { "open" },
        "merged": merge.is_some(),
        "merge_commit_sha": merge.map(|merge| merge.to_string().repeat(40)),
        "mergeable": serde_json::Value::Null,
        "head": {"sha": head.to_string().repeat(40)},
        "base": {"ref": "main"}
    });
    ("200 OK", body.to_string())
}

fn found(body: String) -> Response {
    ("200 OK", body)
}

fn trunk(head: char) -> Response {
    found(serde_json::json!({"object": {"sha": head.to_string().repeat(40)}}).to_string())
}

fn compare(status: &str, merge_base: char) -> Response {
    found(
        serde_json::json!({
            "status": status,
            "merge_base_commit": {"sha": merge_base.to_string().repeat(40)}
        })
        .to_string(),
    )
}

/// The lower layer merged onto the frozen base, in request order.
fn lower_landed() -> Vec<Response> {
    vec![
        pull(11, LOWER, Some(LOWER_MERGE)),
        found(commit_body(LOWER_MERGE, LOWER_TREE, &[BASE, LOWER])),
        found(commit_body(LOWER, LOWER_TREE, &[BASE])),
    ]
}

/// The upper layer merged onto the lower layer's merge.
fn upper_landed() -> Vec<Response> {
    vec![
        pull(12, UPPER, Some(UPPER_MERGE)),
        found(commit_body(UPPER_MERGE, UPPER_TREE, &[LOWER_MERGE, UPPER])),
        found(commit_body(UPPER, UPPER_TREE, &[LOWER])),
    ]
}

fn recover(responses: Vec<Response>) -> (PortResult<Option<StackLandingReceipt>>, Vec<String>) {
    let (endpoint, server) = serve(responses);
    let client = GithubLandingClient::new_fixture(token(), 5000, &endpoint)
        .unwrap_or_else(|_| unreachable!());
    let (plan, publication) = stack();
    let result = client.recover_stack_landed(&plan, &publication);
    (result, server.join().unwrap_or_else(|_| unreachable!()))
}

fn outcome(result: PortResult<Option<StackLandingReceipt>>) -> String {
    match result {
        Ok(_) => "ok".to_string(),
        Err(PortFailure::Domain(diagnostic)) => format!("refused:{}", diagnostic.code),
        Err(PortFailure::Unknown(failure)) => format!("unknown:{}", failure.code),
        Err(other) => format!("{other:?}"),
    }
}

#[test]
// r[verify kiln.github.stack] r[verify kiln.implementation.stack.gate.scenario.integration]
fn recovers_exact_stacked_merges_as_a_bottom_prefix_without_mutation() {
    let mut whole = lower_landed();
    whole.extend(upper_landed());
    whole.push(trunk(UPPER_MERGE));
    let (result, requests) = recover(whole);
    let receipt = result
        .unwrap_or_else(|error| unreachable!("{error:?}"))
        .unwrap_or_else(|| unreachable!());
    assert!(receipt.admitted.complete);
    assert_eq!(receipt.admitted.integrated_layers, 2);
    assert_eq!(
        receipt
            .integration
            .layers
            .get(1)
            .map(|layer| layer.mapping.expected_base.clone()),
        Some(git("commit", LOWER_MERGE))
    );
    assert!(requests.iter().all(|request| request.starts_with("GET ")));

    // Only the lower layer landed, and the trunk has since moved past it.
    let mut prefix = lower_landed();
    prefix.extend([
        pull(12, UPPER, None),
        trunk(OTHER),
        compare("ahead", LOWER_MERGE),
    ]);
    let (result, _) = recover(prefix);
    let receipt = result
        .unwrap_or_else(|error| unreachable!("{error:?}"))
        .unwrap_or_else(|| unreachable!());
    assert_eq!(receipt.admitted.integrated_layers, 1);
    assert!(!receipt.admitted.complete);
    assert_eq!(receipt.trunk_head, OTHER.to_string().repeat(40));

    let (result, requests) = recover(vec![pull(11, LOWER, None), pull(12, UPPER, None)]);
    assert_eq!(result.ok(), Some(None));
    assert_eq!(requests.len(), 2);
}

#[test]
// r[verify kiln.implementation.stack.gate.scenario.integration]
fn rewritten_heads_wrong_parents_and_out_of_order_merges_are_never_integrated() {
    let mut rebased = lower_landed();
    rebased.push(pull(12, OTHER, Some(UPPER_MERGE)));
    // The upper layer landed on the frozen base, not on the lower merge.
    let mut skipped = lower_landed();
    skipped.extend([
        pull(12, UPPER, Some(UPPER_MERGE)),
        found(commit_body(UPPER_MERGE, UPPER_TREE, &[BASE, UPPER])),
        found(commit_body(UPPER, UPPER_TREE, &[LOWER])),
    ]);
    let mut diverged = lower_landed();
    diverged.extend(upper_landed());
    diverged.extend([trunk(OTHER), compare("diverged", BASE)]);
    for (responses, expected) in [
        (rebased, "refused:github_stack_layer_head_changed"),
        (skipped, "refused:github_stack_merge_mapping"),
        (
            vec![pull(11, LOWER, None), pull(12, UPPER, Some(UPPER_MERGE))],
            "unknown:github_stack_out_of_order",
        ),
        (diverged, "unknown:github_stack_trunk_unknown"),
    ] {
        let (result, requests) = recover(responses);
        assert_eq!(outcome(result), expected);
        assert!(requests.iter().all(|request| request.starts_with("GET ")));
    }
}

#[test]
// r[verify kiln.implementation.stack.gate.scenario.lower]
fn a_refused_or_unknown_lower_observation_holds_every_higher_layer() {
    let ready = Ok(LandingObservation {
        pull: PullObservation {
            number: 12,
            author: "author".to_string(),
            state: "open".to_string(),
            merged: false,
            mergeable: Some(true),
            head_sha: UPPER.to_string().repeat(40),
            base_ref: "main".to_string(),
        },
        checks: Vec::new(),
        reviews: Vec::new(),
        base_head: LOWER.to_string().repeat(40),
    });
    let stale = Err(domain(
        "github_landing_pull",
        "landing.pull",
        "pull request moved",
    ));
    let late = Err(PortFailure::TimedOut(InfrastructureFailure {
        code: "github_landing_transport".to_string(),
        message: "observation timed out".to_string(),
    }));
    for (lower, gate) in [
        (
            &stale,
            LayerGate::Blocked {
                layer: 2,
                code: "github_landing_pull".to_string(),
            },
        ),
        (
            &late,
            LayerGate::Unknown {
                layer: 2,
                code: "github_landing_transport".to_string(),
            },
        ),
    ] {
        let states = [
            LayerState::Integrated,
            layer_state(lower),
            layer_state(&ready),
        ];
        let gates = stack_gate(&states).unwrap_or_else(|_| unreachable!());
        assert_eq!(gates.first(), Some(&LayerGate::Integrated));
        assert_eq!(gates.get(1), Some(&gate));
        assert_eq!(gates.get(2), Some(&gate));
    }
}
