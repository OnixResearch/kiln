use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use super::*;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

fn scope(root: &str, graphs: &[GraphKind]) -> ProjectScope {
    ProjectScope {
        root: root.to_string(),
        include: strings(&["**"]),
        exclude: Vec::new(),
        depends_on: Vec::new(),
        graphs: graphs.iter().copied().collect(),
    }
}

fn admitted(project_id: &str, scope: &ProjectScope) -> AdmittedScope {
    admit_scope(project_id, scope).unwrap_or_else(|error| unreachable!("{error}"))
}

fn project(project_id: &str, scope: Option<&ProjectScope>) -> ProjectInput {
    ProjectInput {
        project_id: project_id.to_string(),
        scope: scope.map(|scope| admitted(project_id, scope)),
    }
}

fn changed(paths: &[&str]) -> ChangedFiles {
    ChangedFiles::exact(strings(paths))
}

fn reasons<'a>(set: &'a AffectedSet, project_id: &str) -> &'a [Reason] {
    match set.decision(project_id).map(|decision| &decision.outcome) {
        Some(Outcome::Affected { reasons }) => reasons,
        other => unreachable!("{project_id} not affected: {other:?}"),
    }
}

fn skipped(set: &AffectedSet, project_id: &str) -> bool {
    set.decision(project_id).map(|decision| &decision.outcome)
        == Some(&Outcome::Skipped {
            reason: SkipReason::NoChangesInScope,
        })
}

fn graph_reason(graph: GraphKind, from: &str, via: &str) -> Reason {
    Reason::Graph {
        graph,
        from_package: from.to_string(),
        via_package: via.to_string(),
    }
}

fn package(id: &str, dir: &str) -> Package {
    Package {
        id: id.to_string(),
        dir: dir.to_string(),
    }
}

/// Crates `a`, `b` (depends on `a`), `c` (depends on `b`) and unrelated `u`.
fn cargo_graph() -> GraphFacts {
    GraphFacts {
        kind: GraphKind::Cargo,
        result: GraphResult::Available(BuildGraph {
            packages: vec![
                package("a", "crates/a"),
                package("b", "crates/b"),
                package("c", "crates/c"),
                package("u", "crates/u"),
            ],
            edges: vec![
                ("crates/b".to_string(), "crates/a".to_string()),
                ("crates/c".to_string(), "crates/b".to_string()),
            ],
            roots_touching_all: strings(&["Cargo.lock", "Cargo.toml"]),
        }),
    }
}

fn cargo_projects() -> Vec<ProjectInput> {
    ["a", "b", "c", "u"]
        .into_iter()
        .map(|name| {
            project(
                name,
                Some(&scope(&format!("crates/{name}"), &[GraphKind::Cargo])),
            )
        })
        .collect()
}

// r[verify kiln.monorepo.scope]
#[test]
fn admission_accepts_canonical_scopes_and_applies_defaults() {
    let whole = admitted("whole", &scope("", &[]));
    assert_eq!(whole.root(), "");
    assert!(whole.contains("README.md"));
    assert!(whole.contains("crates/api/src/lib.rs"));

    let mut api = scope("crates/api", &[GraphKind::Cargo, GraphKind::Nix]);
    api.include = strings(&["src/**", "Cargo.toml", "src/**"]);
    api.exclude = strings(&["src/**/*.md"]);
    api.depends_on = strings(&["proto/**"]);
    let api = admitted("api", &api);
    assert_eq!(api.project_id(), "api");
    assert_eq!(api.include().len(), 2, "duplicates collapse");
    assert!(api.contains("crates/api/src/lib.rs"));
    assert!(api.contains("crates/api/Cargo.toml"));
    assert!(
        !api.contains("crates/api/src/docs/notes.md"),
        "exclude wins"
    );
    assert!(!api.contains("crates/api/build.rs"), "not included");
    assert!(!api.contains("crates/api2/src/lib.rs"), "sibling prefix");
    assert!(!api.contains("crates/api"), "root itself is a directory");
    assert!(!api.contains("proto/api.proto"), "declared is not in scope");
}

// r[verify kiln.monorepo.scope]
#[test]
fn admission_rejects_every_invalid_field_naming_project_and_field() {
    let long = "a".repeat(MAX_SCOPE_PATTERN_BYTES + 1);
    let many: Vec<String> = (0..=MAX_SCOPE_PATTERNS)
        .map(|index| format!("p{index}"))
        .collect();
    let cases: Vec<(&str, ProjectScope, &str, &str)> = vec![
        ("api", scope("../api", &[]), "scope.root", "invalid_root"),
        (
            "api",
            scope("crates/../api", &[]),
            "scope.root",
            "invalid_root",
        ),
        ("api", scope("/abs", &[]), "scope.root", "invalid_root"),
        ("api", scope("a//b", &[]), "scope.root", "invalid_root"),
        ("api", scope("./a", &[]), "scope.root", "invalid_root"),
        ("api", scope("a/", &[]), "scope.root", "invalid_root"),
        ("api", scope("a\\b", &[]), "scope.root", "invalid_root"),
        ("api", scope("a\0b", &[]), "scope.root", "invalid_root"),
        ("api", scope("crates/*", &[]), "scope.root", "invalid_root"),
        ("api", scope(&long, &[]), "scope.root", "bound_exceeded"),
        ("", scope("a", &[]), "id", "invalid_project_id"),
        (
            "api",
            ProjectScope {
                include: many.clone(),
                ..scope("a", &[])
            },
            "scope.include",
            "bound_exceeded",
        ),
        (
            "api",
            ProjectScope {
                exclude: many.clone(),
                ..scope("a", &[])
            },
            "scope.exclude",
            "bound_exceeded",
        ),
        (
            "api",
            ProjectScope {
                depends_on: many,
                ..scope("a", &[])
            },
            "scope.depends_on",
            "bound_exceeded",
        ),
        (
            "api",
            ProjectScope {
                include: vec!["src/**".to_string(), long.clone()],
                ..scope("a", &[])
            },
            "scope.include[1]",
            "bound_exceeded",
        ),
        (
            "api",
            ProjectScope {
                include: strings(&["../x"]),
                ..scope("a", &[])
            },
            "scope.include[0]",
            "invalid_pattern",
        ),
        (
            "api",
            ProjectScope {
                exclude: strings(&["/x"]),
                ..scope("a", &[])
            },
            "scope.exclude[0]",
            "invalid_pattern",
        ),
        (
            "api",
            ProjectScope {
                depends_on: strings(&["a/./b"]),
                ..scope("a", &[])
            },
            "scope.depends_on[0]",
            "invalid_pattern",
        ),
        (
            "api",
            ProjectScope {
                include: strings(&["src/**.rs"]),
                ..scope("a", &[])
            },
            "scope.include[0]",
            "invalid_pattern",
        ),
        (
            "api",
            ProjectScope {
                include: strings(&[""]),
                ..scope("a", &[])
            },
            "scope.include[0]",
            "invalid_pattern",
        ),
    ];
    for (project_id, scope, field, code) in cases {
        let error = admit_scope(project_id, &scope).err();
        let Some(error) = error else {
            unreachable!("{scope:?} admitted");
        };
        assert_eq!(error.project_id, project_id);
        assert_eq!(error.field, field, "{scope:?}");
        assert_eq!(error.code, code, "{scope:?}");
    }
    let at_limit = ProjectScope {
        include: (0..MAX_SCOPE_PATTERNS)
            .map(|index| format!("p{index}"))
            .collect(),
        root: "a".repeat(MAX_SCOPE_PATTERN_BYTES),
        ..scope("", &[])
    };
    assert!(admit_scope("api", &at_limit).is_ok());
}

// r[verify kiln.monorepo.scope]
#[test]
fn scope_identity_is_stable_order_independent_and_bound_to_project() {
    let mut first = scope("crates/api", &[GraphKind::Nix, GraphKind::Cargo]);
    first.include = strings(&["src/**", "Cargo.toml"]);
    first.depends_on = strings(&["proto/**", "schema/**"]);
    let mut reordered = first.clone();
    reordered.include = strings(&["Cargo.toml", "src/**", "Cargo.toml"]);
    reordered.depends_on = strings(&["schema/**", "proto/**"]);
    let identity = admitted("api", &first).identity().clone();
    assert_eq!(&identity, admitted("api", &first).identity());
    assert_eq!(&identity, admitted("api", &reordered).identity());
    assert_ne!(&identity, admitted("web", &first).identity());
    for changed in [
        ProjectScope {
            root: "crates/web".to_string(),
            ..first.clone()
        },
        ProjectScope {
            exclude: strings(&["src/**"]),
            ..first.clone()
        },
        ProjectScope {
            graphs: [GraphKind::Cargo].into_iter().collect(),
            ..first.clone()
        },
        ProjectScope {
            depends_on: Vec::new(),
            ..first.clone()
        },
        // A pattern moved between lists must change the identity.
        ProjectScope {
            include: strings(&["src/**"]),
            exclude: strings(&["Cargo.toml"]),
            ..first.clone()
        },
    ] {
        assert_ne!(
            &identity,
            admitted("api", &changed).identity(),
            "{changed:?}"
        );
    }
}

// r[verify kiln.monorepo.changed]
#[test]
fn changed_files_are_exact_within_bounds_and_truncate_beyond() {
    let exact = ChangedFiles::exact(strings(&["b", "a", "b"]));
    assert_eq!(
        exact,
        ChangedFiles::Exact(["a", "b"].into_iter().map(String::from).collect())
    );
    assert_eq!(exact.summary(), ChangedSummary::ExactCount(2));
    let at_count: Vec<String> = (0..MAX_CHANGED_PATHS)
        .map(|index| format!("f{index}"))
        .collect();
    assert!(matches!(
        ChangedFiles::exact(at_count.clone()),
        ChangedFiles::Exact(paths) if paths.len() == MAX_CHANGED_PATHS
    ));
    let mut over_count = at_count;
    over_count.push("extra".to_string());
    assert_eq!(
        ChangedFiles::exact(over_count),
        ChangedFiles::Fallback(ChangedFallback::Truncated)
    );
    let chunk = MAX_CHANGED_PATH_BYTES / 4;
    let at_bytes: Vec<String> = (0..4)
        .map(|index| format!("{index}{}", "x".repeat(chunk - 1)))
        .collect();
    assert!(matches!(
        ChangedFiles::exact(at_bytes.clone()),
        ChangedFiles::Exact(_)
    ));
    let mut over_bytes = at_bytes;
    over_bytes.push("y".to_string());
    assert_eq!(
        ChangedFiles::exact(over_bytes),
        ChangedFiles::Fallback(ChangedFallback::Truncated)
    );
    assert_eq!(
        ChangedFiles::Fallback(ChangedFallback::NoBase).summary(),
        ChangedSummary::Fallback(ChangedFallback::NoBase)
    );
}

// r[verify kiln.monorepo.affected]
#[test]
fn path_reason_samples_and_counts_changes_in_scope_only() {
    let mut api = scope("crates/api", &[]);
    api.exclude = strings(&["**/*.md"]);
    let projects = [
        project("api", Some(&api)),
        project("web", Some(&scope("web", &[]))),
    ];
    let files: Vec<String> = (0..7)
        .map(|index| format!("crates/api/src/m{index}.rs"))
        .collect();
    let mut all = files.clone();
    all.extend(strings(&[
        "crates/api/README.md",
        "crates/api2/lib.rs",
        "docs/x.md",
    ]));
    let set = affected(&projects, &ChangedFiles::exact(all), &[]);
    assert_eq!(
        reasons(&set, "api"),
        [Reason::Path {
            sample: files.into_iter().take(MAX_REASON_SAMPLE).collect(),
            count: 7,
        }]
    );
    assert!(skipped(&set, "web"));
    let only_excluded = affected(&projects, &changed(&["crates/api/README.md"]), &[]);
    assert!(skipped(&only_excluded, "api"));
    let empty = affected(&projects, &changed(&[]), &[]);
    assert!(skipped(&empty, "api") && skipped(&empty, "web"));
}

// r[verify kiln.monorepo.affected]
#[test]
fn declared_dependencies_affect_with_pattern_and_sample() {
    let mut web = scope("web", &[]);
    web.depends_on = strings(&["proto/**", "schema/*.json"]);
    let projects = [project("web", Some(&web))];
    let set = affected(
        &projects,
        &changed(&["proto/a.proto", "schema/x.json", "schema/deep/y.json"]),
        &[],
    );
    assert_eq!(
        reasons(&set, "web"),
        [
            Reason::Declared {
                pattern: "proto/**".to_string(),
                sample: strings(&["proto/a.proto"]),
            },
            Reason::Declared {
                pattern: "schema/*.json".to_string(),
                sample: strings(&["schema/x.json"]),
            },
        ]
    );
    assert!(skipped(
        &affected(&projects, &changed(&["schema/deep/y.json"]), &[]),
        "web"
    ));
}

// r[verify kiln.monorepo.affected.scenario.graph]
#[test]
fn graph_dependants_are_affected_and_unrelated_crates_skipped() {
    let set = affected(
        &cargo_projects(),
        &changed(&["crates/a/src/lib.rs"]),
        &[cargo_graph()],
    );
    assert_eq!(
        reasons(&set, "a"),
        [
            Reason::Path {
                sample: strings(&["crates/a/src/lib.rs"]),
                count: 1,
            },
            graph_reason(GraphKind::Cargo, "a", "a"),
        ]
    );
    assert_eq!(
        reasons(&set, "b"),
        [graph_reason(GraphKind::Cargo, "a", "b")]
    );
    assert_eq!(
        reasons(&set, "c"),
        [graph_reason(GraphKind::Cargo, "a", "c")],
        "transitive dependant"
    );
    assert!(skipped(&set, "u"));
    // Dependencies are not affected by their dependants.
    let downstream = affected(
        &cargo_projects(),
        &changed(&["crates/c/src/lib.rs"]),
        &[cargo_graph()],
    );
    assert!(skipped(&downstream, "a") && skipped(&downstream, "b"));
    // A project that does not consult the graph ignores it.
    let plain = [project("b", Some(&scope("crates/b", &[])))];
    assert!(skipped(
        &affected(&plain, &changed(&["crates/a/src/lib.rs"]), &[cargo_graph()]),
        "b"
    ));
    // A file outside every package changes no package.
    assert!(
        affected(
            &cargo_projects(),
            &changed(&["docs/a.md"]),
            &[cargo_graph()]
        )
        .decisions
        .iter()
        .all(|decision| !decision.is_affected())
    );
}

// r[verify kiln.monorepo.affected]
#[test]
fn deepest_package_owns_a_file_and_root_projects_see_every_package() {
    let graph = GraphFacts {
        kind: GraphKind::Nix,
        result: GraphResult::Available(BuildGraph {
            packages: vec![
                package(".", ""),
                package("tools", "tools"),
                package("tools/inner", "tools/inner"),
            ],
            edges: vec![(String::new(), "tools".to_string())],
            roots_touching_all: strings(&["flake.lock", "flake.nix"]),
        }),
    };
    let projects = [
        project("everything", Some(&scope("", &[GraphKind::Nix]))),
        project("tools", Some(&scope("tools", &[GraphKind::Nix]))),
        project("docs", Some(&scope("docs", &[GraphKind::Nix]))),
    ];
    let inner = affected(
        &projects,
        &changed(&["tools/inner/x.nix"]),
        &[graph.clone()],
    );
    assert_eq!(
        reasons(&inner, "tools"),
        [
            Reason::Path {
                sample: strings(&["tools/inner/x.nix"]),
                count: 1,
            },
            graph_reason(GraphKind::Nix, "tools/inner", "tools/inner"),
        ]
    );
    assert!(skipped(&inner, "docs"), "root package is not under docs");
    let tools = affected(
        &projects,
        &changed(&["tools/default.nix"]),
        &[graph.clone()],
    );
    assert_eq!(
        reasons(&tools, "everything"),
        [
            Reason::Path {
                sample: strings(&["tools/default.nix"]),
                count: 1,
            },
            graph_reason(GraphKind::Nix, "tools", "."),
            graph_reason(GraphKind::Nix, "tools", "tools"),
        ]
    );
    let roots = affected(&projects, &changed(&["flake.lock"]), &[graph]);
    for project_id in ["docs", "tools"] {
        assert_eq!(
            reasons(&roots, project_id),
            [graph_reason(GraphKind::Nix, "flake.lock", ALL_PACKAGES)]
        );
    }
}

// r[verify kiln.monorepo.affected]
#[test]
fn roots_touching_all_affect_only_projects_consulting_that_graph() {
    let mut projects = cargo_projects();
    projects.push(project("docs", Some(&scope("docs", &[]))));
    projects.push(project("nixy", Some(&scope("nixy", &[GraphKind::Nix]))));
    let nix = GraphFacts {
        kind: GraphKind::Nix,
        result: GraphResult::Available(BuildGraph::default()),
    };
    let set = affected(&projects, &changed(&["Cargo.lock"]), &[cargo_graph(), nix]);
    for project_id in ["a", "b", "c", "u"] {
        assert_eq!(
            reasons(&set, project_id),
            [graph_reason(GraphKind::Cargo, "Cargo.lock", ALL_PACKAGES)]
        );
    }
    assert!(skipped(&set, "docs") && skipped(&set, "nixy"));
}

// r[verify kiln.monorepo.affected]
#[test]
fn overlapping_and_unscoped_projects() {
    let mut narrow = scope("crates", &[]);
    narrow.include = strings(&["api/**"]);
    let projects = [
        project("wide", Some(&scope("crates", &[]))),
        project("narrow", Some(&narrow)),
        project("legacy", None),
    ];
    let set = affected(&projects, &changed(&["crates/api/lib.rs"]), &[]);
    assert!(set.decisions.iter().all(ProjectDecision::is_affected));
    assert_eq!(reasons(&set, "legacy"), [Reason::Unscoped]);
    let none = affected(&projects, &changed(&[]), &[]);
    assert_eq!(reasons(&none, "legacy"), [Reason::Unscoped]);
    assert!(skipped(&none, "wide") && skipped(&none, "narrow"));
    let fallback = affected(
        &projects,
        &ChangedFiles::Fallback(ChangedFallback::NoBase),
        &[],
    );
    assert_eq!(reasons(&fallback, "legacy"), [Reason::Unscoped]);
}

// r[verify kiln.monorepo.affected.scenario.fallback]
#[test]
fn uncertain_facts_never_skip() {
    let projects = cargo_projects();
    for fallback in [
        ChangedFallback::NoBase,
        ChangedFallback::Truncated,
        ChangedFallback::UnsupportedProvider,
        ChangedFallback::ReadError,
    ] {
        let set = affected(
            &projects,
            &ChangedFiles::Fallback(fallback),
            &[cargo_graph()],
        );
        for decision in &set.decisions {
            assert_eq!(
                reasons(&set, &decision.project_id),
                [Reason::Fallback {
                    reason: fallback.as_str().to_string(),
                }]
            );
        }
    }
    let mut mixed = cargo_projects();
    mixed.push(project("docs", Some(&scope("docs", &[]))));
    let unparsable = GraphFacts {
        kind: GraphKind::Cargo,
        result: GraphResult::Unavailable("unparsable manifest Cargo.toml".to_string()),
    };
    let set = affected(&mixed, &changed(&["crates/a/src/lib.rs"]), &[unparsable]);
    assert_eq!(
        reasons(&set, "a"),
        [
            Reason::Path {
                sample: strings(&["crates/a/src/lib.rs"]),
                count: 1,
            },
            Reason::Fallback {
                reason: "graph_unavailable: cargo: unparsable manifest Cargo.toml".to_string(),
            },
        ]
    );
    assert_eq!(
        reasons(&set, "u"),
        [Reason::Fallback {
            reason: "graph_unavailable: cargo: unparsable manifest Cargo.toml".to_string(),
        }]
    );
    assert!(
        skipped(&set, "docs"),
        "projects not using the graph keep precision"
    );
    let missing = affected(&projects, &changed(&["x"]), &[]);
    assert_eq!(
        reasons(&missing, "u"),
        [Reason::Fallback {
            reason: "graph_unavailable: cargo: graph facts not supplied".to_string(),
        }]
    );
    let mut malformed = cargo_graph();
    if let GraphResult::Available(graph) = &mut malformed.result {
        graph
            .edges
            .push(("crates/u".to_string(), "crates/ghost".to_string()));
    }
    let malformed = affected(&projects, &changed(&["x"]), &[malformed]);
    assert!(matches!(
        reasons(&malformed, "u"),
        [Reason::Fallback { reason }] if reason.starts_with("graph_unavailable: cargo: edge")
    ));
    let mut duplicate = cargo_graph();
    if let GraphResult::Available(graph) = &mut duplicate.result {
        graph.packages.push(package("again", "crates/a"));
    }
    assert!(matches!(
        reasons(&affected(&projects, &changed(&["x"]), &[duplicate]), "u"),
        [Reason::Fallback { .. }]
    ));
    let oversized = GraphFacts {
        kind: GraphKind::Cargo,
        result: GraphResult::Available(BuildGraph {
            packages: (0..=MAX_GRAPH_PACKAGES)
                .map(|index| package(&format!("p{index}"), &format!("p{index}")))
                .collect(),
            ..BuildGraph::default()
        }),
    };
    assert!(matches!(
        reasons(&affected(&projects, &changed(&["x"]), &[oversized]), "u"),
        [Reason::Fallback { .. }]
    ));
}

// r[verify kiln.monorepo.affected]
#[test]
fn affected_set_identity_is_deterministic_and_order_independent() {
    let files = changed(&["crates/a/src/lib.rs"]);
    let forward = cargo_projects();
    let mut reversed = cargo_projects();
    reversed.reverse();
    let first = affected(&forward, &files, &[cargo_graph()]);
    let second = affected(&reversed, &files, &[cargo_graph()]);
    assert_eq!(first, second);
    let ids: Vec<&str> = first
        .decisions
        .iter()
        .map(|decision| decision.project_id.as_str())
        .collect();
    assert_eq!(ids, ["a", "b", "c", "u"]);
    assert_ne!(
        first.identity,
        affected(
            &forward,
            &changed(&["crates/a/src/main.rs"]),
            &[cargo_graph()]
        )
        .identity,
        "exact changed facts are bound"
    );
    let mut rescoped = cargo_projects();
    rescoped.retain(|project| project.project_id != "u");
    let mut wider = scope("crates/u", &[GraphKind::Cargo]);
    wider.exclude = strings(&["**/*.md"]);
    rescoped.push(project("u", Some(&wider)));
    assert_ne!(
        first.identity,
        affected(&rescoped, &files, &[cargo_graph()]).identity,
        "scope identities are bound even when decisions match"
    );
    assert_ne!(
        affected(
            &forward,
            &ChangedFiles::Fallback(ChangedFallback::NoBase),
            &[]
        )
        .identity,
        affected(
            &forward,
            &ChangedFiles::Fallback(ChangedFallback::ReadError),
            &[]
        )
        .identity
    );
}

#[test]
fn affected_set_json_uses_the_shared_wire_shape() {
    let projects = [
        project("api", Some(&scope("crates/a", &[GraphKind::Cargo]))),
        project("web", Some(&scope("web", &[]))),
    ];
    let set = affected(&projects, &changed(&["crates/a/x.rs"]), &[cargo_graph()]);
    let json = serde_json::to_value(&set).unwrap_or_default();
    assert_eq!(
        json.get("decisions"),
        Some(&serde_json::json!([
            {
                "project_id": "api",
                "outcome": "affected",
                "reasons": [
                    {"kind": "path", "sample": ["crates/a/x.rs"], "count": 1},
                    {"kind": "graph", "graph": "cargo", "from_package": "a", "via_package": "a"}
                ]
            },
            {"project_id": "web", "outcome": "skipped", "reason": "no_changes_in_scope"}
        ]))
    );
    assert_eq!(
        json.get("identity").and_then(serde_json::Value::as_str),
        Some(set.identity.as_str())
    );
    let decoded: AffectedSet = serde_json::from_value(json).unwrap_or_else(|_| unreachable!());
    assert_eq!(decoded, set);
    let scope_json = serde_json::to_value(admitted("api", &scope("crates/a", &[GraphKind::Cargo])))
        .unwrap_or_default();
    assert_eq!(
        scope_json.get("graphs"),
        Some(&serde_json::json!(["cargo"]))
    );
    let parsed: ProjectScope = serde_json::from_str(r#"{"root":"crates/a","graphs":["nix"]}"#)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(parsed.include, strings(&["**"]));
    assert!(serde_json::from_str::<ProjectScope>(r#"{"root":"a","extra":1}"#).is_err());
    assert_eq!(
        serde_json::to_value(ChangedSummary::ExactCount(12)).unwrap_or_default(),
        serde_json::json!({"exact_count": 12})
    );
    assert_eq!(
        serde_json::to_value(ChangedSummary::Fallback(ChangedFallback::NoBase)).unwrap_or_default(),
        serde_json::json!({"fallback": "no_base"})
    );
}
