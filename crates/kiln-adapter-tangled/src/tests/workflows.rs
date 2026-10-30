use std::collections::BTreeSet;

use super::*;
use crate::when::*;
use crate::workflows::*;

fn engines() -> Vec<EngineBinding> {
    profile_value().engines
}

fn map(yaml: &str) -> Result<ReviewedWorkflow, String> {
    review(
        ".tangled/workflows/ci.yml",
        yaml.as_bytes(),
        &engines(),
        65_536,
    )
}

// r[verify kiln.tangled.workflows]
#[test]
fn reviewed_subset_maps_every_supported_field() {
    let yaml = "when:\n  - event: [\"push\", \"manual\"]\n    branch: [\"main\", \"release-*\"]\n    paths: [\"src/**\"]\n  - event: push\n    tag: \"v*\"\n  - event: pull_request\n    branch: main\n    types: [opened, merged]\nengine: microvm\nimage: nixos\ndependencies: [go, nodejs, go]\nclone:\n  depth: 0\n  submodules: false\n  tags: false\nenvironment:\n  NODE_ENV: production\n  RETRIES: 3\nsteps:\n  - name: build\n    command: go build ./...\n    environment:\n      GOOS: linux\n";
    let workflow = ok(map(yaml));
    assert_eq!(workflow.id, "ci.yml");
    assert_eq!(
        workflow.dependencies,
        vec!["go".to_string(), "nodejs".to_string()]
    );
    assert_eq!(
        workflow.clone,
        CloneOptions {
            skip: false,
            depth: 1,
            tags: false
        }
    );
    assert_eq!(
        workflow.environment.get("RETRIES").map(String::as_str),
        Some("3")
    );
    assert_eq!(workflow.steps.len(), 1);
    assert!(workflow.notes.is_empty());
    assert_eq!(
        workflow.triggers,
        vec![
            // One rule per listed event, in `EventKind` order.
            TriggerRule {
                event: EventKind::Manual,
                filter: TriggerFilter::default()
            },
            TriggerRule {
                event: EventKind::Push,
                filter: TriggerFilter {
                    branches: vec!["main".into(), "release-*".into()],
                    tags: vec![],
                    types: vec![],
                    paths: vec!["src/**".into()]
                },
            },
            TriggerRule {
                event: EventKind::Push,
                filter: TriggerFilter {
                    tags: vec!["v*".into()],
                    ..TriggerFilter::default()
                }
            },
            TriggerRule {
                event: EventKind::PullRequest,
                filter: TriggerFilter {
                    branches: vec!["main".into()],
                    types: vec![PullRequestType::Opened, PullRequestType::Merged],
                    ..TriggerFilter::default()
                },
            },
        ]
    );
    assert_eq!(
        workflow.trigger_nsids(),
        vec![
            crate::nsid::MANUAL,
            crate::nsid::PULL_REQUEST,
            crate::nsid::PUSH
        ]
    );
    // Without `when`, Tangled runs for every event.
    let bare = ok(map(
        "engine: microvm\nsteps:\n  - name: x\n    command: make\n",
    ));
    let kinds: BTreeSet<EventKind> = bare.triggers.iter().map(|rule| rule.event).collect();
    assert_eq!(
        kinds,
        BTreeSet::from([EventKind::Push, EventKind::PullRequest, EventKind::Manual])
    );
    assert_eq!(bare.notes.len(), 1, "implicit submodules are noted");
}

// r[verify kiln.tangled.workflows.scenario.unsupported]
#[test]
fn unsupported_fields_and_when_keys_fail_with_their_path() {
    let base = "engine: microvm\nsteps:\n  - name: x\n    command: make\n";
    let cases = [
        (
            "when:\n  - event: schedule\n    schedule:\n      - cron: \"H 2 * * *\"\n",
            "$.when[0].schedule",
        ),
        ("when:\n  - event: schedule\n", "$.when[0].event"),
        (
            "when:\n  - event: push\n    branches: main\n",
            "$.when[0].branches",
        ),
        (
            "when:\n  - event: pull_request\n    tag: v1\n",
            "$.when[0].tag",
        ),
        (
            "when:\n  - event: push\n    branch: main\n    types: [opened]\n",
            "$.when[0].types",
        ),
        (
            "when:\n  - event: push\n",
            "$.when[0]: push conditions need branch or tag",
        ),
        (
            "when:\n  - event: manual\n    paths: [\"x/**\"]\n",
            "manual runs ignore",
        ),
        (
            "when:\n  - event: pull_request\n    types: [edited]\n",
            "$.when[0].types",
        ),
        (
            "when:\n  - event: push\n    tag: \"release-**\"\n",
            "$.when[0].tag",
        ),
        (
            "when:\n  - event: push\n    branch: \"ma?n\"\n",
            "$.when[0].branch",
        ),
        ("when: []\n", "$.when"),
        (
            "cache:\n  - key: go\n    paths: [x]\n",
            "$.cache: caches are not mapped",
        ),
        ("runs_on: [arm64]\n", "$.runs_on"),
        ("audience: sts.amazonaws.com\n", "$.audience"),
        ("services:\n  postgresql: true\n", "$.services"),
        ("registry:\n  nixpkgs: x\n", "$.registry"),
        ("clone:\n  submodules: true\n", "$.clone.submodules"),
        ("clone:\n  depth: -1\n", "$.clone.depth"),
        ("dependencies: [rustc]\n", "$.dependencies"),
        (
            "dependencies:\n  nixpkgs: [go]\n  custom: [x]\n",
            "$.dependencies.custom",
        ),
        ("image: alpine\n", "$.image"),
        ("environment:\n  1BAD: x\n", "$.environment.1BAD"),
        ("environment:\n  NESTED: [a]\n", "$.environment.NESTED"),
        ("unknown: true\n", "$.unknown"),
    ];
    for (extra, expected) in cases {
        let yaml = format!("{base}{extra}");
        let message = err(map(&yaml));
        assert!(message.contains(expected), "{extra:?} gave {message}");
        assert!(
            message.starts_with(".tangled/workflows/ci.yml"),
            "{message}"
        );
    }
    assert!(err(map("engine: nixery\n")).contains("$.engine"));
    assert!(err(map("steps: []\n")).contains("$.engine: required"));
    assert!(err(map(&format!("{base}steps2: x\n"))).contains("$.steps2"));
    assert!(
        err(map("engine: microvm\nsteps:\n  - name: x\n    run: true\n"))
            .contains("$.steps[0].run")
    );
    assert!(err(map("- a\n- b\n")).contains("expected a mapping"));
    assert!(err(map("engine: microvm\nengine: nixery\n")).contains("YAML"));
    assert!(
        review(
            ".tangled/workflows/ci.yml",
            &[0xff, 0xfe],
            &engines(),
            65_536
        )
        .is_err()
    );
    assert!(
        review(
            ".tangled/workflows/sub/ci.yml",
            base.as_bytes(),
            &engines(),
            65_536
        )
        .is_err()
    );
    assert!(review(".tangled/workflows/ci.yml", base.as_bytes(), &engines(), 4).is_err());
    assert_eq!(workflow_id(".tangled/workflows/a.yaml"), Some("a.yaml"));
    assert_eq!(workflow_id(".tangled/workflows/.hidden.yml"), None);
    assert_eq!(workflow_id(".tangled/workflows/readme.md"), None);
}

#[test]
fn when_rules_match_tangled_semantics() {
    let changed =
        |paths: &[&str]| ChangedFiles::Exact(paths.iter().map(|path| path.to_string()).collect());
    let unknown = ChangedFiles::Fallback(ChangedFallback::NoBase);
    let push = |branches: &[&str], tags: &[&str], paths: &[&str]| TriggerRule {
        event: EventKind::Push,
        filter: TriggerFilter {
            branches: branches.iter().map(|v| v.to_string()).collect(),
            tags: tags.iter().map(|v| v.to_string()).collect(),
            types: vec![],
            paths: paths.iter().map(|v| v.to_string()).collect(),
        },
    };
    let main = TriggerEvent::Push {
        reference: "refs/heads/main",
    };
    let release = TriggerEvent::Push {
        reference: "refs/heads/release-1/x",
    };
    let tag = TriggerEvent::Push {
        reference: "refs/tags/v1.2",
    };
    assert!(rule_matches(&push(&["main"], &[], &[]), &main, &unknown));
    assert!(
        !rule_matches(&push(&["release-*"], &[], &[]), &release, &unknown),
        "* stays within a segment"
    );
    assert!(rule_matches(
        &push(&["release-*/**"], &[], &[]),
        &release,
        &unknown
    ));
    assert!(
        !rule_matches(&push(&[], &["v*"], &[]), &main, &unknown),
        "tag-only rules exclude branches"
    );
    assert!(rule_matches(&push(&[], &["v*"], &[]), &tag, &unknown));
    assert!(
        !rule_matches(&push(&["main"], &[], &[]), &tag, &unknown),
        "branch-only rules exclude tags"
    );
    assert!(rule_matches(
        &push(&["main"], &[], &["docs/**"]),
        &main,
        &changed(&["docs/a/b.md"])
    ));
    assert!(!rule_matches(
        &push(&["main"], &[], &["docs/**"]),
        &main,
        &changed(&["src/a.rs"])
    ));
    assert!(
        rule_matches(&push(&["main"], &[], &["docs/**"]), &main, &unknown),
        "unknown changes run"
    );
    let pull = |types: Vec<PullRequestType>| TriggerRule {
        event: EventKind::PullRequest,
        filter: TriggerFilter {
            branches: vec!["main".into()],
            types,
            ..TriggerFilter::default()
        },
    };
    let opened = TriggerEvent::PullRequest {
        action: PullRequestType::Opened,
        base_branch: "main",
    };
    let closed = TriggerEvent::PullRequest {
        action: PullRequestType::Closed,
        base_branch: "main",
    };
    let other_base = TriggerEvent::PullRequest {
        action: PullRequestType::Opened,
        base_branch: "dev",
    };
    assert!(rule_matches(&pull(vec![]), &opened, &unknown));
    assert!(
        !rule_matches(&pull(vec![]), &closed, &unknown),
        "default types exclude closed"
    );
    assert!(rule_matches(
        &pull(vec![PullRequestType::Closed]),
        &closed,
        &unknown
    ));
    assert!(!rule_matches(&pull(vec![]), &other_base, &unknown));
    assert!(
        !rule_matches(&pull(vec![]), &main, &unknown),
        "event kinds differ"
    );
}
